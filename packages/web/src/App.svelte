<script lang="ts">
  import { onMount } from "svelte";
  import CustomerEnvironmentView from "./lib/customer/CustomerEnvironmentView.svelte";
  import CustomerLanView from "./lib/customer/CustomerLanView.svelte";
  import ApplianceConfigView from "./lib/config/ApplianceConfigView.svelte";
  import ConnectionConfigView from "./lib/config/ConnectionConfigView.svelte";
  import OfficeEnvironmentView from "./lib/office/OfficeEnvironmentView.svelte";
  import FactoryOverview from "./lib/process/FactoryOverview.svelte";
  import ProcessAreaView from "./lib/process/ProcessAreaView.svelte";
  import ProcessCanvas from "./lib/process/canvas/ProcessCanvas.svelte";
  import LocationOverview from "./lib/shared/LocationOverview.svelte";
  import RegionMap from "./lib/shared/RegionMap.svelte";
  import {
    findAppliance,
    findConnection,
  } from "./lib/config/appliance-config";
  import { findProcessArea } from "./lib/process/process-model";
  import type {
    EnvironmentRoute,
    ApplianceConfigRoute,
    ConnectionConfigRoute,
    PlaceId,
    ProcessAreaRoute,
    ViewMode,
  } from "./lib/shared/types";

  type ArchitectureRoute = PlaceId | EnvironmentRoute | ProcessAreaRoute;
  type ConfigRoute = ApplianceConfigRoute | ConnectionConfigRoute;
  type ActiveRoute = ArchitectureRoute | ConfigRoute | null;

  let activeRoute: ActiveRoute = null;
  let configHistory: (ArchitectureRoute | ConfigRoute)[] = [];
  let viewMode: ViewMode = "logical";

  function syncRoute() {
    const route = window.location.hash.slice(1);
    const processAreaKey = route.startsWith("factory/process/")
      ? route.slice("factory/process/".length)
      : "";
    const isProcessArea = processAreaKey !== "" && findProcessArea(processAreaKey) !== null;
    const applianceId = route.startsWith("config/appliances/")
      ? route.slice("config/appliances/".length)
      : "";
    const connectionId = route.startsWith("config/connections/")
      ? route.slice("config/connections/".length)
      : "";
    const isApplianceConfig = applianceId !== "" && findAppliance(applianceId) !== null;
    const isConnectionConfig =
      connectionId !== "" && findConnection(connectionId) !== null;

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
          : isApplianceConfig
            ? (route as ApplianceConfigRoute)
            : isConnectionConfig
              ? (route as ConnectionConfigRoute)
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

  function openApplianceConfig(applianceId: string) {
    if (!findAppliance(applianceId)) return;
    openConfigRoute(`config/appliances/${applianceId}` as ApplianceConfigRoute);
  }

  function openConnectionConfig(connectionId: string) {
    if (!findConnection(connectionId)) return;
    openConfigRoute(
      `config/connections/${connectionId}` as ConnectionConfigRoute,
    );
  }

  function openConfigRoute(route: ConfigRoute) {
    if (activeRoute && activeRoute !== route) {
      configHistory = [...configHistory, activeRoute];
    }
    activeRoute = route;
    window.location.hash = route;
  }

  function returnFromConfig() {
    const previous = configHistory.at(-1);
    if (previous) {
      configHistory = configHistory.slice(0, -1);
      activeRoute = previous;
      window.location.hash = previous;
      return;
    }
    returnToMap();
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
{:else if activeRoute.startsWith("config/appliances/")}
  <ApplianceConfigView
    applianceId={activeRoute.slice("config/appliances/".length)}
    onBack={returnFromConfig}
    onOpenConnection={openConnectionConfig}
  />
{:else if activeRoute.startsWith("config/connections/")}
  <ConnectionConfigView
    connectionId={activeRoute.slice("config/connections/".length)}
    onBack={returnFromConfig}
    onOpenAppliance={openApplianceConfig}
  />
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
    onOpenAppliance={openApplianceConfig}
  />
{:else if activeRoute.startsWith("factory/process/")}
  <ProcessAreaView
    bind:viewMode
    routeKey={activeRoute.slice("factory/process/".length)}
    onBack={returnToProcess}
    onOpenAppliance={openApplianceConfig}
  />
{:else if activeRoute === "customer/customer-lan"}
  <CustomerLanView
    bind:viewMode
    onBack={returnToCustomer}
    onOpenAppliance={openApplianceConfig}
  />
{:else if
  activeRoute === "customer/customer-edge" ||
  activeRoute === "customer/public-web-path"}
  <CustomerEnvironmentView
    bind:viewMode
    environment={activeRoute === "customer/customer-edge" ? "edge" : "public-web-path"}
    onBack={returnToCustomer}
    onOpenAppliance={openApplianceConfig}
  />
{:else if
  activeRoute === "office/it-dmz" ||
  activeRoute === "office/business-it" ||
  activeRoute === "office/operations-intelligence"}
  <OfficeEnvironmentView
    bind:viewMode
    environment={activeRoute.slice("office/".length) as "it-dmz" | "business-it" | "operations-intelligence"}
    onBack={returnToOffice}
    onOpenAppliance={openApplianceConfig}
  />
{:else if activeRoute === "factory/ot-dmz"}
  <OfficeEnvironmentView
    bind:viewMode
    environment="ot-dmz"
    siteLabel="Factory"
    onBack={returnToFactory}
    onOpenAppliance={openApplianceConfig}
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
