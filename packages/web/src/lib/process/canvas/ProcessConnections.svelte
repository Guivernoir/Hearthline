<svg
  class="connections"
  viewBox="0 0 1640 1080"
  aria-hidden="true"
>
  <defs>
    <marker
      id="network-arrow"
      markerWidth="8"
      markerHeight="8"
      refX="7"
      refY="4"
      orient="auto"
    >
      <path d="M0,0 L8,4 L0,8 Z" fill="#4d7379"></path>
    </marker>
    <marker
      id="material-arrow"
      markerWidth="9"
      markerHeight="9"
      refX="8"
      refY="4.5"
      orient="auto"
    >
      <path d="M0,0 L9,4.5 L0,9 Z" fill="#c76432"></path>
    </marker>
  </defs>

  <g class="process-physical-background">
    <rect class="process-site-shell" x="25" y="90" width="1590" height="850"></rect>
    <rect class="process-support-gallery" x="25" y="90" width="1590" height="175"></rect>
    <rect class="process-production-hall" x="25" y="280" width="1590" height="660"></rect>
    <rect class="process-utility-corridor" x="45" y="615" width="1550" height="82"></rect>

    <rect class="process-cell process-cell-wet" x="40" y="370" width="285" height="230"></rect>
    <rect class="process-cell process-cell-forming" x="350" y="370" width="285" height="230"></rect>
    <rect class="process-cell process-cell-drying" x="660" y="370" width="285" height="230"></rect>
    <rect class="process-cell process-cell-thermal" x="970" y="370" width="285" height="230"></rect>
    <rect class="process-cell process-cell-finish" x="1280" y="370" width="285" height="230"></rect>

    <rect class="process-cell process-cell-logistics" x="40" y="710" width="285" height="210"></rect>
    <rect class="process-cell process-cell-quality" x="350" y="710" width="285" height="210"></rect>
    <rect class="process-cell process-cell-thermal" x="660" y="710" width="285" height="210"></rect>
    <rect class="process-cell process-cell-quality" x="970" y="710" width="285" height="210"></rect>
    <rect class="process-cell process-cell-thermal" x="1280" y="710" width="285" height="210"></rect>

    <path class="process-overhead-utility" d="M80 640 H1560"></path>
    <path class="process-floor-rail" d="M75 585 H1565 M75 902 H1565"></path>
    <path class="process-loading-bay" d="M55 750 H100 V885 H55 M265 750 H310 V885 H265"></path>
    <path class="process-exhaust-stack" d="M1120 365 V305 H1150 V365 M1430 705 V645 H1460 V705"></path>

    <text class="process-physical-label" x="50" y="112">CONTROL AND SITE SERVICES GALLERY</text>
    <text class="process-physical-label" x="50" y="310">CERAMICS PRODUCTION HALL</text>
    <text class="process-physical-label" x="70" y="666">UTILITY, MATERIAL, AND MAINTENANCE CORRIDOR</text>
    <text class="process-detail-label" x="70" y="736">PACKING / DISPATCH BAY</text>
    <text class="process-detail-label" x="1320" y="736">PRIMARY FIRING BAY</text>
  </g>

  <g class="network-connections">
    <path d="M260 165 H350" marker-end="url(#network-arrow)"></path>
    <path d="M570 165 H660" marker-end="url(#network-arrow)"></path>
    <path d="M880 165 H970" marker-end="url(#network-arrow)"></path>
    <path d="M1190 165 H1280" marker-end="url(#network-arrow)"></path>
    <path d="M1080 225 V355"></path>
    <path d="M180 355 H1420"></path>
    <path d="M180 690 H1420"></path>
    <path d="M1265 355 V690"></path>
    <path d="M180 355 V430"></path>
    <path d="M490 355 V430"></path>
    <path d="M800 355 V430"></path>
    <path d="M1110 355 V430"></path>
    <path d="M1420 355 V430"></path>
    <path d="M180 690 V760"></path>
    <path d="M490 690 V760"></path>
    <path d="M800 690 V760"></path>
    <path d="M1110 690 V760"></path>
    <path d="M1420 690 V760"></path>
  </g>

  <g class="material-connections">
    <path d="M215 495 H455" marker-end="url(#material-arrow)"></path>
    <path d="M525 495 H765" marker-end="url(#material-arrow)"></path>
    <path d="M835 495 H1075" marker-end="url(#material-arrow)"></path>
    <path d="M1145 495 H1385" marker-end="url(#material-arrow)"></path>
    <path d="M1420 530 V790" marker-end="url(#material-arrow)"></path>
    <path d="M1385 825 H1145" marker-end="url(#material-arrow)"></path>
    <path d="M1075 825 H835" marker-end="url(#material-arrow)"></path>
    <path d="M765 825 H525" marker-end="url(#material-arrow)"></path>
    <path d="M455 825 H215" marker-end="url(#material-arrow)"></path>
  </g>

  <g class="physical-support-connections">
    <path class="site-support-link" d="M177 156 H433"></path>
    <path class="site-support-link" d="M487 156 H743"></path>
    <path d="M797 156 H1053"></path>
    <path d="M1107 156 H1363"></path>
  </g>

  <text x="74" y="335" class="line-label network-label">CONTROL NETWORK</text>
  <text x="72" y="405" class="line-label material-label">MATERIAL FLOW</text>
</svg>
