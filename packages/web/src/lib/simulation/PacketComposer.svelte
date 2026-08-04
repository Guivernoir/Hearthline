<script lang="ts">
  import type { ScenarioPacket } from "./simulation-api";

  export let packet: ScenarioPacket;
  export let locked = false;
  export let onSubmit: () => void = () => {};

  $: udp = packet.transport.protocol === "udp" ? packet.transport : null;
  $: tcp = packet.transport.protocol === "tcp" ? packet.transport : null;
  $: icmp = packet.transport.protocol === "icmp-echo" ? packet.transport : null;
  $: dns = packet.application.kind === "dns-query" ? packet.application : null;
  $: httpRequest =
    packet.application.kind === "http-request" ? packet.application : null;
  $: serviceApplication =
    packet.application.kind === "service" ? packet.application : null;

  function updateHttpBody(value: string) {
    if (packet.application.kind !== "http-request") return;
    packet.application.body = value || null;
    packet.application.body_bytes = new TextEncoder().encode(value).length;
    packet = { ...packet };
  }
</script>

<form onsubmit={(event) => { event.preventDefault(); onSubmit(); }}>
  <fieldset disabled={locked}>
    <legend>IPv4</legend>
    <label>
      <span>Source address</span>
      <input type="text" bind:value={packet.source_ip} spellcheck="false" />
    </label>
    <label>
      <span>Destination address</span>
      <input type="text" bind:value={packet.destination_ip} spellcheck="false" />
    </label>
    <div class="field-pair">
      <label><span>TTL</span><input type="number" min="1" max="255" bind:value={packet.ttl} /></label>
      <label><span>Wire bytes</span><input type="number" min="64" max="65535" bind:value={packet.wire_length_bytes} /></label>
    </div>
  </fieldset>

  <fieldset disabled={locked}>
    <legend>Transport / {packet.transport.protocol}</legend>
    {#if udp}
      <div class="field-pair">
        <label><span>Source port</span><input type="number" min="1" max="65535" bind:value={udp.source_port} /></label>
        <label><span>Destination port</span><input type="number" min="1" max="65535" bind:value={udp.destination_port} /></label>
      </div>
    {:else if tcp}
      <div class="field-pair">
        <label><span>Source port</span><input type="number" min="1" max="65535" bind:value={tcp.source_port} /></label>
        <label><span>Destination port</span><input type="number" min="1" max="65535" bind:value={tcp.destination_port} /></label>
      </div>
      <div class="flag-row">
        <label><input type="checkbox" bind:checked={tcp.syn} /> SYN</label>
        <label><input type="checkbox" bind:checked={tcp.ack} /> ACK</label>
        <label><input type="checkbox" bind:checked={tcp.fin} /> FIN</label>
        <label><input type="checkbox" bind:checked={tcp.rst} /> RST</label>
      </div>
    {:else if icmp}
      <div class="field-pair">
        <label><span>Identifier</span><input type="number" min="0" max="65535" bind:value={icmp.identifier} /></label>
        <label><span>Sequence</span><input type="number" min="0" max="65535" bind:value={icmp.sequence} /></label>
      </div>
    {/if}
  </fieldset>

  {#if dns}
    <fieldset disabled={locked}>
      <legend>DNS query</legend>
      <label><span>Record name</span><input type="text" bind:value={dns.name} spellcheck="false" /></label>
    </fieldset>
  {:else if httpRequest}
    <fieldset disabled={locked}>
      <legend>HTTP request</legend>
      <label>
        <span>Method</span>
        <select bind:value={httpRequest.method}>
          <option value="get">GET</option><option value="head">HEAD</option>
          <option value="post">POST</option><option value="put">PUT</option>
          <option value="patch">PATCH</option><option value="delete">DELETE</option>
          <option value="options">OPTIONS</option>
        </select>
      </label>
      <label><span>Host</span><input type="text" bind:value={httpRequest.host} spellcheck="false" /></label>
      <label><span>Path</span><input type="text" bind:value={httpRequest.path} spellcheck="false" /></label>
      <label>
        <span>Request body</span>
        <textarea maxlength="256" spellcheck="false" value={httpRequest.body ?? ""} oninput={(event) => updateHttpBody(event.currentTarget.value)}></textarea>
      </label>
      <label><span>Body bytes</span><input type="number" min="0" disabled={httpRequest.body !== null} bind:value={httpRequest.body_bytes} /></label>
    </fieldset>
  {:else if serviceApplication}
    <fieldset disabled={locked}>
      <legend>Application service</legend>
      <label><span>Service</span><input type="text" bind:value={serviceApplication.service} spellcheck="false" /></label>
    </fieldset>
  {/if}
</form>
