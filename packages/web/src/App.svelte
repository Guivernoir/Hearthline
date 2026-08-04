<script lang="ts">
  import { onMount } from "svelte";
  import CustomerEnvironmentView from "./lib/customer/CustomerEnvironmentView.svelte";
  import CustomerLanView from "./lib/customer/CustomerLanView.svelte";
  import ApplianceConfigView from "./lib/config/ApplianceConfigView.svelte";
  import ConnectionConfigView from "./lib/config/ConnectionConfigView.svelte";
  import OfficeEnvironmentView from "./lib/office/OfficeEnvironmentView.svelte";
  import SecurityConsoleView from "./lib/office/SecurityConsoleView.svelte";
  import FactoryOverview from "./lib/process/FactoryOverview.svelte";
  import HmiView from "./lib/process/hmi/HmiView.svelte";
  import ProcessAreaView from "./lib/process/ProcessAreaView.svelte";
  import ProcessCanvas from "./lib/process/canvas/ProcessCanvas.svelte";
  import LocationOverview from "./lib/shared/LocationOverview.svelte";
  import RegionMap from "./lib/shared/RegionMap.svelte";
  import SimulationWorkspace from "./lib/simulation/SimulationWorkspace.svelte";
  import WorkstationView from "./lib/workstation/WorkstationView.svelte";
  import {
    findAppliance,
    findConnection,
    isInteractiveHmi,
    isInteractiveSecurityConsole,
    isInteractiveWorkstation,
  } from "./lib/config/appliance-config";
  import { findProcessArea } from "./lib/process/process-model";
  import type {
    EnvironmentRoute,
    HmiRoute,
    ApplianceConfigRoute,
    ConnectionConfigRoute,
    PlaceId,
    ProcessAreaRoute,
    SecurityConsoleRoute,
    ViewMode,
    WorkstationRoute,
  } from "./lib/shared/types";

  type ArchitectureRoute = PlaceId | EnvironmentRoute | ProcessAreaRoute;
  type ConfigRoute = ApplianceConfigRoute | ConnectionConfigRoute;
  type DetailRoute =
    | ConfigRoute
    | WorkstationRoute
    | HmiRoute
    | SecurityConsoleRoute;
  type ActiveRoute = ArchitectureRoute | DetailRoute | "simulations" | null;

  let activeRoute: ActiveRoute = null;
  let detailHistory: (ArchitectureRoute | DetailRoute | "simulations")[] = [];
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
    const workstationId = route.startsWith("workstations/")
      ? route.slice("workstations/".length)
      : "";
    const hmiId = route.startsWith("hmis/")
      ? route.slice("hmis/".length)
      : "";
    const securityConsoleId = route.startsWith("security/")
      ? route.slice("security/".length)
      : "";
    const isApplianceConfig = applianceId !== "" && findAppliance(applianceId) !== null;
    const isConnectionConfig =
      connectionId !== "" && findConnection(connectionId) !== null;
    const isWorkstation =
      workstationId !== "" && isInteractiveWorkstation(workstationId);
    const isHmi = hmiId !== "" && isInteractiveHmi(hmiId);
    const isSecurityConsole = securityConsoleId !== "" &&
      isInteractiveSecurityConsole(securityConsoleId);

    activeRoute = route === "simulations"
      ? "simulations"
      : route === "customer" ||
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
              : isWorkstation
                ? (route as WorkstationRoute)
                : isHmi
                  ? (route as HmiRoute)
                  : isSecurityConsole
                    ? (route as SecurityConsoleRoute)
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

  function openSimulations() {
    activeRoute = "simulations";
    window.location.hash = "simulations";
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
    openDetailRoute(`config/appliances/${applianceId}` as ApplianceConfigRoute);
  }

  function openConnectionConfig(connectionId: string) {
    if (!findConnection(connectionId)) return;
    openDetailRoute(
      `config/connections/${connectionId}` as ConnectionConfigRoute,
    );
  }

  function openWorkstation(applianceId: string) {
    if (!isInteractiveWorkstation(applianceId)) return;
    openDetailRoute(`workstations/${applianceId}` as WorkstationRoute);
  }

  function openHmi(applianceId: string) {
    if (!isInteractiveHmi(applianceId)) return;
    openDetailRoute(`hmis/${applianceId}` as HmiRoute);
  }

  function openSecurityConsole(applianceId: string) {
    if (!isInteractiveSecurityConsole(applianceId)) return;
    openDetailRoute(`security/${applianceId}` as SecurityConsoleRoute);
  }

  function openDetailRoute(route: DetailRoute) {
    if (activeRoute && activeRoute !== route) {
      detailHistory = [...detailHistory, activeRoute];
    }
    activeRoute = route;
    window.location.hash = route;
  }

  function returnFromDetail() {
    const previous = detailHistory.at(-1);
    if (previous) {
      detailHistory = detailHistory.slice(0, -1);
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

{#key activeRoute}
{#if activeRoute === null}
  <RegionMap
    bind:viewMode
    onEnter={enterPlace}
    onOpenSimulations={openSimulations}
  />
{:else if activeRoute === "simulations"}
  <SimulationWorkspace
    onBack={returnToMap}
    onOpenAppliance={openApplianceConfig}
  />
{:else if activeRoute.startsWith("config/appliances/")}
  <ApplianceConfigView
    applianceId={activeRoute.slice("config/appliances/".length)}
    onBack={returnFromDetail}
    onOpenConnection={openConnectionConfig}
  />
{:else if activeRoute.startsWith("config/connections/")}
  <ConnectionConfigView
    connectionId={activeRoute.slice("config/connections/".length)}
    onBack={returnFromDetail}
    onOpenAppliance={openApplianceConfig}
  />
{:else if activeRoute.startsWith("workstations/")}
  <WorkstationView
    applianceId={activeRoute.slice("workstations/".length)}
    onBack={returnFromDetail}
    onOpenConfig={openApplianceConfig}
  />
{:else if activeRoute.startsWith("hmis/")}
  <HmiView
    applianceId={activeRoute.slice("hmis/".length)}
    onBack={returnFromDetail}
    onOpenConfig={openApplianceConfig}
  />
{:else if activeRoute.startsWith("security/")}
  <SecurityConsoleView
    applianceId={activeRoute.slice("security/".length)}
    onBack={returnFromDetail}
    onOpenConfig={openApplianceConfig}
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
    onOpenHmi={openHmi}
  />
{:else if activeRoute === "customer/customer-lan"}
  <CustomerLanView
    bind:viewMode
    onBack={returnToCustomer}
    onOpenAppliance={openApplianceConfig}
    onOpenWorkstation={openWorkstation}
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
    onOpenSecurityConsole={openSecurityConsole}
    onOpenWorkstation={openWorkstation}
  />
{:else if activeRoute === "factory/ot-dmz"}
  <OfficeEnvironmentView
    bind:viewMode
    environment="ot-dmz"
    siteLabel="Factory"
    onBack={returnToFactory}
    onOpenAppliance={openApplianceConfig}
    onOpenSecurityConsole={openSecurityConsole}
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
{/key}
