use std::collections::BTreeSet;
use std::error::Error;
use std::fs;
use std::path::{Path, PathBuf};

use syn::visit::{self, Visit};
use walkdir::WalkDir;

struct RuntimeVisitor {
    forbidden: BTreeSet<String>,
    float_literal: bool,
}

impl<'ast> Visit<'ast> for RuntimeVisitor {
    fn visit_path(&mut self, path: &'ast syn::Path) {
        if path
            .segments
            .first()
            .is_some_and(|segment| segment.ident == "std")
        {
            self.forbidden.insert("std path".into());
        }
        visit::visit_path(self, path);
    }

    fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
        if let Some(segment) = path.path.segments.last() {
            let name = segment.ident.to_string();
            if [
                "Box", "Rc", "Arc", "HashMap", "HashSet", "VecDeque", "String",
            ]
            .contains(&name.as_str())
            {
                self.forbidden.insert(format!("heap-backed type {name}"));
            }
            if name == "Vec"
                && !path
                    .path
                    .segments
                    .iter()
                    .any(|part| part.ident == "heapless")
            {
                self.forbidden.insert("heap-backed type Vec".into());
            }
        }
        visit::visit_type_path(self, path);
    }

    fn visit_lit_float(&mut self, literal: &'ast syn::LitFloat) {
        self.float_literal = true;
        visit::visit_lit_float(self, literal);
    }
}

pub fn verify() -> Result<(), Box<dyn Error>> {
    let repository = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let runtime_roots = [
        repository.join("packages/crates/hearthline-model/src"),
        repository.join("packages/crates/hearthline-engine/src"),
    ];
    let host_deterministic_roots = [
        repository.join("packages/crates/hearthline-project/src"),
        repository.join("packages/crates/hearthline-sim/src"),
        repository.join("packages/crates/hearthline-operator/src"),
    ];
    let allowlist =
        fixed_point_allowlist(&repository.join("project/standards/fixed-point-allowlist.txt"))?;
    let mut failures = Vec::new();
    for root in runtime_roots {
        for entry in WalkDir::new(root).into_iter().filter_map(Result::ok) {
            if !entry.file_type().is_file()
                || entry.path().extension().and_then(|value| value.to_str()) != Some("rs")
            {
                continue;
            }
            let source = fs::read_to_string(entry.path())?;
            let syntax = syn::parse_file(&source)
                .map_err(|error| format!("{}: {error}", entry.path().display()))?;
            let mut visitor = RuntimeVisitor {
                forbidden: BTreeSet::new(),
                float_literal: false,
            };
            visitor.visit_file(&syntax);
            for violation in &visitor.forbidden {
                failures.push(format!(
                    "{} uses {violation}",
                    relative(&repository, entry.path())
                ));
            }
            if contains_float(&syntax) || visitor.float_literal {
                let path = relative(&repository, entry.path());
                if !allowlist.contains(&path) {
                    failures.push(format!(
                        "{path} introduces unquantized floating-point state"
                    ));
                }
            }
        }
    }
    for root in host_deterministic_roots {
        for entry in rust_files(&root) {
            let source = fs::read_to_string(entry.path())?;
            let syntax = syn::parse_file(&source)
                .map_err(|error| format!("{}: {error}", entry.path().display()))?;
            for declaration in state_float_declarations(&syntax) {
                failures.push(format!(
                    "{} retains floating-point state in {declaration}",
                    relative(&repository, entry.path()),
                ));
            }
        }
    }
    for allowed in &allowlist {
        let path = repository.join(allowed);
        if !path.exists() {
            failures.push(format!("fixed-point allowlist contains missing {allowed}"));
        }
    }
    if failures.is_empty() {
        Ok(())
    } else {
        Err(format!("runtime policy violations:\n- {}", failures.join("\n- ")).into())
    }
}

fn rust_files(root: &Path) -> impl Iterator<Item = walkdir::DirEntry> {
    WalkDir::new(root)
        .into_iter()
        .filter_map(Result::ok)
        .filter(|entry| {
            entry.file_type().is_file()
                && entry.path().extension().and_then(|value| value.to_str()) == Some("rs")
        })
}

fn fixed_point_allowlist(path: &Path) -> Result<BTreeSet<String>, Box<dyn Error>> {
    Ok(fs::read_to_string(path)?
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .map(str::to_owned)
        .collect())
}

fn contains_float(file: &syn::File) -> bool {
    struct FloatVisitor(bool);
    impl<'ast> Visit<'ast> for FloatVisitor {
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            if path.path.is_ident("f32") || path.path.is_ident("f64") {
                self.0 = true;
            }
            visit::visit_type_path(self, path);
        }
    }
    let mut visitor = FloatVisitor(false);
    visitor.visit_file(file);
    visitor.0
}

fn state_float_declarations(file: &syn::File) -> BTreeSet<String> {
    struct StateVisitor {
        violations: BTreeSet<String>,
    }

    impl<'ast> Visit<'ast> for StateVisitor {
        fn visit_item_struct(&mut self, item: &'ast syn::ItemStruct) {
            let retains_float = item
                .fields
                .iter()
                .any(|field| type_contains_float(&field.ty));
            if retains_float {
                if projection_boundary(&item.attrs) {
                    if !derives_serialize(&item.attrs) {
                        self.violations.insert(format!(
                            "projection boundary struct {} without Serialize",
                            item.ident
                        ));
                    }
                } else {
                    self.violations.insert(format!("struct {}", item.ident));
                }
            }
            visit::visit_item_struct(self, item);
        }

        fn visit_item_enum(&mut self, item: &'ast syn::ItemEnum) {
            if item
                .variants
                .iter()
                .flat_map(|variant| &variant.fields)
                .any(|field| type_contains_float(&field.ty))
            {
                self.violations.insert(format!("enum {}", item.ident));
            }
            visit::visit_item_enum(self, item);
        }

        fn visit_item_const(&mut self, item: &'ast syn::ItemConst) {
            if type_contains_float(&item.ty) {
                self.violations.insert(format!("const {}", item.ident));
            }
            visit::visit_item_const(self, item);
        }

        fn visit_item_static(&mut self, item: &'ast syn::ItemStatic) {
            if type_contains_float(&item.ty) {
                self.violations.insert(format!("static {}", item.ident));
            }
            visit::visit_item_static(self, item);
        }

        fn visit_item_type(&mut self, item: &'ast syn::ItemType) {
            if type_contains_float(&item.ty) {
                self.violations.insert(format!("type alias {}", item.ident));
            }
            visit::visit_item_type(self, item);
        }

        fn visit_impl_item_const(&mut self, item: &'ast syn::ImplItemConst) {
            if type_contains_float(&item.ty) {
                self.violations
                    .insert(format!("associated const {}", item.ident));
            }
            visit::visit_impl_item_const(self, item);
        }
    }

    let mut visitor = StateVisitor {
        violations: BTreeSet::new(),
    };
    visitor.visit_file(file);
    visitor.violations
}

fn type_contains_float(ty: &syn::Type) -> bool {
    struct FloatTypeVisitor(bool);
    impl<'ast> Visit<'ast> for FloatTypeVisitor {
        fn visit_type_path(&mut self, path: &'ast syn::TypePath) {
            if path.path.is_ident("f32") || path.path.is_ident("f64") {
                self.0 = true;
            }
            visit::visit_type_path(self, path);
        }
    }
    let mut visitor = FloatTypeVisitor(false);
    visitor.visit_type(ty);
    visitor.0
}

fn projection_boundary(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        if !attribute.path().is_ident("doc") {
            return false;
        }
        let syn::Meta::NameValue(meta) = &attribute.meta else {
            return false;
        };
        let syn::Expr::Lit(expression) = &meta.value else {
            return false;
        };
        let syn::Lit::Str(value) = &expression.lit else {
            return false;
        };
        value.value().trim() == "hearthline:projection-boundary"
    })
}

fn derives_serialize(attributes: &[syn::Attribute]) -> bool {
    attributes.iter().any(|attribute| {
        attribute.path().is_ident("derive")
            && attribute
                .parse_args_with(
                    syn::punctuated::Punctuated::<syn::Path, syn::Token![,]>::parse_terminated,
                )
                .is_ok_and(|paths| {
                    paths.iter().any(|path| {
                        path.segments
                            .last()
                            .is_some_and(|item| item.ident == "Serialize")
                    })
                })
    })
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .to_string_lossy()
        .replace('\\', "/")
}

#[cfg(test)]
mod tests {
    use super::state_float_declarations;

    #[test]
    fn state_policy_rejects_nested_float_storage() {
        let syntax = syn::parse_file(
            "struct State { value: Option<[f64; 2]> } enum Mode { Value(f32) } const LIMIT: f64 = 1.0;",
        )
        .unwrap();
        let violations = state_float_declarations(&syntax);
        assert!(violations.contains("struct State"));
        assert!(violations.contains("enum Mode"));
        assert!(violations.contains("const LIMIT"));
    }

    #[test]
    fn state_policy_allows_conversion_functions_and_marked_projections() {
        let syntax = syn::parse_file(
            r#"
            fn quantize(value: f64) -> i64 { value as i64 }
            #[derive(Serialize)]
            #[doc = "hearthline:projection-boundary"]
            struct Projection { value: f64 }
            "#,
        )
        .unwrap();
        assert!(state_float_declarations(&syntax).is_empty());
    }

    #[test]
    fn projection_marker_requires_serialization() {
        let syntax = syn::parse_file(
            r#"#[doc = "hearthline:projection-boundary"] struct State { value: f64 }"#,
        )
        .unwrap();
        assert!(
            state_float_declarations(&syntax)
                .iter()
                .any(|item| item.contains("without Serialize"))
        );
    }
}
