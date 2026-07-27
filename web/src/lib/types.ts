export type ViewMode = "physical" | "logical";

export type PlaceId = "customer" | "office" | "factory";

export type EnvironmentRoute =
  | "customer/customer-lan"
  | "customer/customer-edge"
  | "customer/public-web-path"
  | "office/it-dmz"
  | "office/business-it"
  | "office/operations-intelligence"
  | "factory/ot-dmz"
  | "factory/process";

export type ProcessAreaRoute = `factory/process/${string}`;
