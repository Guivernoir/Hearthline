<script lang="ts">
  import { onMount } from "svelte";
  import CustomerEnvironmentView from "./lib/CustomerEnvironmentView.svelte";
  import CustomerLanView from "./lib/CustomerLanView.svelte";
  import FactoryOverview from "./lib/FactoryOverview.svelte";
  import LocationOverview from "./lib/LocationOverview.svelte";
  import OfficeEnvironmentView from "./lib/OfficeEnvironmentView.svelte";
  import ProcessAreaView from "./lib/ProcessAreaView.svelte";
  import ProcessCanvas from "./lib/ProcessCanvas.svelte";
  import RegionMap from "./lib/RegionMap.svelte";
  import { findProcessArea } from "./lib/process-model";
  import type {
    EnvironmentRoute,
    PlaceId,
    ProcessAreaRoute,
    ViewMode,
  } from "./lib/types";

  type ActiveRoute = PlaceId | EnvironmentRoute | ProcessAreaRoute | null;

  let activeRoute: ActiveRoute = null;
  let viewMode: ViewMode = "logical";

  function syncRoute() {
    const route = window.location.hash.slice(1);
    const processAreaKey = route.startsWith("factory/process/")
      ? route.slice("factory/process/".length)
      : "";
    const isProcessArea = processAreaKey !== "" && findProcessArea(processAreaKey) !== null;

    activeRoute = route === "customer" ||
      route === "office" ||
      route === "factory" ||
      route === "customer/customer-lan" ||
      route === "customer/customer-edge" ||
      route === "customer/public-web-path" ||
      route === "office/it-dmz" ||
      route === "office/business-it" ||
      route === "office/operations-intelligence" ||
      route === "factory/ot-dmz" ||
      route === "factory/process"
        ? (route as PlaceId | EnvironmentRoute)
        : isProcessArea
          ? (route as ProcessAreaRoute)
        : null;
  }

  function enterPlace(place: PlaceId) {
    activeRoute = place;
    window.location.hash = place;
  }

  function returnToMap() {
    activeRoute = null;
    window.history.replaceState(null, "", window.location.pathname + window.location.search);
  }

  function enterCustomerEnvironment(environmentId: string) {
    const routes: Record<string, EnvironmentRoute> = {
      "customer-lan": "customer/customer-lan",
      "customer-edge": "customer/customer-edge",
      "public-service": "customer/public-web-path",
    };
    const route = routes[environmentId];
    if (!route) return;
    activeRoute = route;
    window.location.hash = route;
  }

  function enterOfficeEnvironment(environmentId: string) {
    const routes: Record<string, EnvironmentRoute> = {
      "it-dmz": "office/it-dmz",
      "business-it": "office/business-it",
      "operations-intelligence": "office/operations-intelligence",
    };
    const route = routes[environmentId];
    if (!route) return;
    activeRoute = route;
    window.location.hash = route;
  }

  function returnToCustomer() {
    activeRoute = "customer";
    window.history.replaceState(
      null,
      "",
      `${window.location.pathname}${window.location.search}#customer`,
    );
  }

  function returnToOffice() {
    activeRoute = "office";
    window.history.replaceState(
      null,
      "",
      `${window.location.pathname}${window.location.search}#office`,
    );
  }

  function enterFactoryEnvironment(environmentId: string) {
    const routes: Record<string, EnvironmentRoute> = {
      "ot-dmz": "factory/ot-dmz",
      "process": "factory/process",
    };
    const route = routes[environmentId];
    if (!route) return;
    activeRoute = route;
    window.location.hash = route;
  }

  function returnToFactory() {
    activeRoute = "factory";
    window.history.replaceState(
      null,
      "",
      `${window.location.pathname}${window.location.search}#factory`,
    );
  }

  function enterProcessArea(routeKey: string) {
    const route = `factory/process/${routeKey}` as ProcessAreaRoute;
    activeRoute = route;
    window.location.hash = route;
  }

  function returnToProcess() {
    activeRoute = "factory/process";
    window.history.replaceState(
      null,
      "",
      `${window.location.pathname}${window.location.search}#factory/process`,
    );
  }

  onMount(() => {
    if (window.location.hash === "#office/ot-dmz") {
      window.history.replaceState(
        null,
        "",
        `${window.location.pathname}${window.location.search}#factory/ot-dmz`,
      );
    }
    syncRoute();
    window.addEventListener("hashchange", syncRoute);
    return () => window.removeEventListener("hashchange", syncRoute);
  });
</script>

{#if activeRoute === null}
  <RegionMap bind:viewMode onEnter={enterPlace} />
{:else if activeRoute === "factory"}
  <FactoryOverview
    bind:viewMode
    onBack={returnToMap}
    onEnterEnvironment={enterFactoryEnvironment}
  />
{:else if activeRoute === "factory/process"}
  <ProcessCanvas
    bind:viewMode
    onBack={returnToFactory}
    onEnterArea={enterProcessArea}
  />
{:else if activeRoute.startsWith("factory/process/")}
  <ProcessAreaView
    bind:viewMode
    routeKey={activeRoute.slice("factory/process/".length)}
    onBack={returnToProcess}
  />
{:else if activeRoute === "customer/customer-lan"}
  <CustomerLanView bind:viewMode onBack={returnToCustomer} />
{:else if
  activeRoute === "customer/customer-edge" ||
  activeRoute === "customer/public-web-path"}
  <CustomerEnvironmentView
    bind:viewMode
    environment={activeRoute === "customer/customer-edge" ? "edge" : "public-web-path"}
    onBack={returnToCustomer}
  />
{:else if
  activeRoute === "office/it-dmz" ||
  activeRoute === "office/business-it" ||
  activeRoute === "office/operations-intelligence"}
  <OfficeEnvironmentView
    bind:viewMode
    environment={activeRoute.slice("office/".length) as "it-dmz" | "business-it" | "operations-intelligence"}
    onBack={returnToOffice}
  />
{:else if activeRoute === "factory/ot-dmz"}
  <OfficeEnvironmentView
    bind:viewMode
    environment="ot-dmz"
    siteLabel="Factory"
    onBack={returnToFactory}
  />
{:else if activeRoute === "office" || activeRoute === "customer"}
  <LocationOverview
    bind:viewMode
    place={activeRoute}
    onBack={returnToMap}
    onEnterEnvironment={activeRoute === "office"
      ? enterOfficeEnvironment
      : enterCustomerEnvironment}
  />
{/if}
