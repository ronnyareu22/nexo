<!-- Íconos de línea (24×24, trazo 1.8), un solo juego al estilo de SF Symbols. Toman el color del texto. -->
<script lang="ts" module>
  export type Nombre =
    | 'actualizar'
    | 'atras'
    | 'bluetooth'
    | 'conectar'
    | 'desconectar'
    | 'alerta'
    | 'cerrar'
    | 'volumen'
    | 'silencio'
    | 'microfono'
    | 'check'
    | 'sol'
    | 'sol-bajo'
    | 'teclado'
    | 'luna'
    | 'auto'
    | 'encendido'
    | 'ajustes'
    | 'reloj'
    | 'inicio'
    | 'dispositivos'
    | 'audio'
    | 'pantallas'
    | 'automatizaciones'
    | 'widgets'
    | 'engranaje'
    | 'buscar'
    | 'mas'
    | 'editar'
    | 'papelera'
    | 'chevron'
    | 'chevron-abajo'
    | 'bateria'
    | 'rayo'
    | 'info'
    | 'opciones'
    | 'onda'
    | 'check-circulo'
    | 'remoto'
    | 'portatil'
    | 'captura'
    | 'carpeta'
    | 'portapapeles'
    | 'copiar'
    | 'flecha';
</script>

<script lang="ts">
  let { nombre, tamano = 18 }: { nombre: Nombre; tamano?: number } = $props();
</script>

<svg
  width={tamano}
  height={tamano}
  viewBox="0 0 24 24"
  fill="none"
  stroke="currentColor"
  stroke-width="1.8"
  stroke-linecap="round"
  stroke-linejoin="round"
  aria-hidden="true"
>
  {#if nombre === 'actualizar'}
    <path d="M20 11a8 8 0 0 0-14.6-4.4L4 8" />
    <path d="M4 3.5V8h4.5" />
    <path d="M4 13a8 8 0 0 0 14.6 4.4L20 16" />
    <path d="M20 20.5V16h-4.5" />
  {:else if nombre === 'atras'}
    <path d="M15 18l-6-6 6-6" />
  {:else if nombre === 'bluetooth'}
    <path d="M7 7l10 10-5 4.5V2.5L17 7 7 17" />
  {:else if nombre === 'conectar'}
    <path d="M9 15l6-6" />
    <path d="M11 6.5l1.2-1.2a4.2 4.2 0 0 1 6 6L17 12.5" />
    <path d="M13 17.5l-1.2 1.2a4.2 4.2 0 0 1-6-6L7 11.5" />
  {:else if nombre === 'desconectar'}
    <path d="M11 6.5l1.2-1.2a4.2 4.2 0 0 1 6 6L17 12.5" />
    <path d="M13 17.5l-1.2 1.2a4.2 4.2 0 0 1-6-6L7 11.5" />
    <path d="M4 4l16 16" />
  {:else if nombre === 'alerta'}
    <path d="M12 3.5l9.5 16.5h-19z" />
    <path d="M12 10v4.5" />
    <path d="M12 17.5h.01" />
  {:else if nombre === 'cerrar'}
    <path d="M6 6l12 12M18 6L6 18" />
  {:else if nombre === 'volumen'}
    <path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" />
    <path d="M15.5 9a4 4 0 0 1 0 6" />
    <path d="M18 6.5a7.5 7.5 0 0 1 0 11" />
  {:else if nombre === 'silencio'}
    <path d="M4 9.5h3.5L12 5.5v13l-4.5-4H4z" />
    <path d="M16 10l5 5M21 10l-5 5" />
  {:else if nombre === 'microfono'}
    <rect x="9" y="3" width="6" height="11" rx="3" />
    <path d="M5.5 11a6.5 6.5 0 0 0 13 0" />
    <path d="M12 17.5V21" />
  {:else if nombre === 'check'}
    <path d="M5 12.5l4.5 4.5L19 7.5" />
  {:else if nombre === 'sol'}
    <circle cx="12" cy="12" r="4" />
    <path d="M12 2.5v2M12 19.5v2M4.6 4.6l1.4 1.4M18 18l1.4 1.4M2.5 12h2M19.5 12h2M4.6 19.4L6 18M18 6l1.4-1.4" />
  {:else if nombre === 'sol-bajo'}
    <circle cx="12" cy="12" r="3.5" />
    <path d="M12 5.5v.5M12 18v.5M7.4 7.4l.4.4M16.2 16.2l.4.4M5.5 12h.5M18 12h.5M7.4 16.6l.4-.4M16.2 7.8l.4-.4" />
  {:else if nombre === 'encendido'}
    <path d="M12 3v8.5" />
    <path d="M6.8 6.6a7.5 7.5 0 1 0 10.4 0" />
  {:else if nombre === 'ajustes'}
    <path d="M4 7h10M18 7h2M4 17h4M12 17h8" />
    <circle cx="16" cy="7" r="2" />
    <circle cx="10" cy="17" r="2" />
  {:else if nombre === 'reloj'}
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 7.5V12l3 2" />
  {:else if nombre === 'luna'}
    <path d="M20 14.5A8 8 0 0 1 9.5 4a8 8 0 1 0 10.5 10.5z" />
  {:else if nombre === 'auto'}
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 3.5a8.5 8.5 0 0 1 0 17z" fill="currentColor" stroke="none" />
  {:else if nombre === 'teclado'}
    <rect x="2.5" y="6" width="19" height="12" rx="2.5" />
    <path d="M6 10h.01M9 10h.01M12 10h.01M15 10h.01M18 10h.01M7 14h10" />
  {:else if nombre === 'inicio'}
    <path d="M3.5 11L12 4l8.5 7" />
    <path d="M5.5 9.5V19a1.5 1.5 0 0 0 1.5 1.5h10a1.5 1.5 0 0 0 1.5-1.5V9.5" />
    <path d="M10 20.5V15h4v5.5" />
  {:else if nombre === 'dispositivos'}
    <path d="M13 15H5.5A1.5 1.5 0 0 1 4 13.5v-7A1.5 1.5 0 0 1 5.5 5h11A1.5 1.5 0 0 1 18 6.5V7" />
    <path d="M2 18.5h11" />
    <rect x="15.5" y="9.5" width="6.5" height="11" rx="1.5" />
  {:else if nombre === 'audio'}
    <path d="M3.5 18.5V12a8.5 8.5 0 0 1 17 0v6.5" />
    <path d="M3.5 14H6a2 2 0 0 1 2 2v2.5a2 2 0 0 1-2 2h-.5a2 2 0 0 1-2-2z" />
    <path d="M20.5 14H18a2 2 0 0 0-2 2v2.5a2 2 0 0 0 2 2h.5a2 2 0 0 0 2-2z" />
  {:else if nombre === 'pantallas'}
    <rect x="2.5" y="4" width="19" height="12.5" rx="2" />
    <path d="M12 16.5v4M8.5 20.5h7" />
  {:else if nombre === 'automatizaciones'}
    <path d="M10 4.5Q10.8 11.4 17.5 12.25Q10.8 13.1 10 20Q9.2 13.1 2.5 12.25Q9.2 11.4 10 4.5z" />
    <path d="M18.5 2.5Q18.8 5.2 21.5 5.5Q18.8 5.8 18.5 8.5Q18.2 5.8 15.5 5.5Q18.2 5.2 18.5 2.5z" />
  {:else if nombre === 'widgets'}
    <rect x="3.5" y="3.5" width="7" height="7" rx="2" />
    <rect x="13.5" y="3.5" width="7" height="7" rx="2" />
    <rect x="3.5" y="13.5" width="7" height="7" rx="2" />
    <rect x="13.5" y="13.5" width="7" height="7" rx="2" />
  {:else if nombre === 'engranaje'}
    <path
      d="M10.26 5.01L10.69 2.69A9.4 9.4 0 0 1 13.31 2.69L13.74 5.01A7.2 7.2 0 0 1 15.71 5.83L17.66 4.49A9.4 9.4 0 0 1 19.51 6.34L18.17 8.29A7.2 7.2 0 0 1 18.99 10.26L21.31 10.69A9.4 9.4 0 0 1 21.31 13.31L18.99 13.74A7.2 7.2 0 0 1 18.17 15.71L19.51 17.66A9.4 9.4 0 0 1 17.66 19.51L15.71 18.17A7.2 7.2 0 0 1 13.74 18.99L13.31 21.31A9.4 9.4 0 0 1 10.69 21.31L10.26 18.99A7.2 7.2 0 0 1 8.29 18.17L6.34 19.51A9.4 9.4 0 0 1 4.49 17.66L5.83 15.71A7.2 7.2 0 0 1 5.01 13.74L2.69 13.31A9.4 9.4 0 0 1 2.69 10.69L5.01 10.26A7.2 7.2 0 0 1 5.83 8.29L4.49 6.34A9.4 9.4 0 0 1 6.34 4.49L8.29 5.83A7.2 7.2 0 0 1 10.26 5.01Z"
    />
    <circle cx="12" cy="12" r="3" />
  {:else if nombre === 'buscar'}
    <circle cx="10.5" cy="10.5" r="6.5" />
    <path d="M15.3 15.3l5.2 5.2" />
  {:else if nombre === 'mas'}
    <path d="M12 5v14M5 12h14" />
  {:else if nombre === 'editar'}
    <path d="M4 20l1.1-4.4L16.1 4.6a2.1 2.1 0 0 1 3 3L8.1 18.6z" />
    <path d="M14.5 6.5l3 3" />
  {:else if nombre === 'papelera'}
    <path d="M4 6.5h16" />
    <path d="M9 6.5V5a1.5 1.5 0 0 1 1.5-1.5h3A1.5 1.5 0 0 1 15 5v1.5" />
    <path d="M6 6.5l.9 12.6a1.5 1.5 0 0 0 1.5 1.4h7.2a1.5 1.5 0 0 0 1.5-1.4L18 6.5" />
    <path d="M10 10.5v6M14 10.5v6" />
  {:else if nombre === 'chevron'}
    <path d="M9 6l6 6-6 6" />
  {:else if nombre === 'chevron-abajo'}
    <path d="M6 9l6 6 6-6" />
  {:else if nombre === 'bateria'}
    <rect x="2.5" y="7" width="17" height="10" rx="2.5" />
    <path d="M22 10.5v3" />
  {:else if nombre === 'rayo'}
    <path d="M13 2.5L5 13.5h6l-1 8 8-11h-6z" />
  {:else if nombre === 'info'}
    <circle cx="12" cy="12" r="8.5" />
    <path d="M12 11v5" />
    <path d="M12 7.8h.01" />
  {:else if nombre === 'opciones'}
    <path d="M5.5 12h.01M12 12h.01M18.5 12h.01" stroke-width="3" />
  {:else if nombre === 'onda'}
    <path d="M3 10.5v3M7 8v8M11 5v14M15 8.5v7M19 10v4" />
  {:else if nombre === 'check-circulo'}
    <circle cx="12" cy="12" r="8.5" />
    <path d="M8.3 12.2l2.5 2.5 5-5.2" />
  {:else if nombre === 'remoto'}
    <!-- Dos pantallas, una detrás de la otra (compartir pantalla) -->
    <rect x="2.5" y="8" width="13.5" height="9.5" rx="1.8" />
    <path d="M9.25 17.5v3M6.5 20.5h5.5" />
    <path d="M8 5.5v-.7A1.8 1.8 0 0 1 9.8 3h9.9a1.8 1.8 0 0 1 1.8 1.8v6.9a1.8 1.8 0 0 1-1.8 1.8H18.5" />
  {:else if nombre === 'portatil'}
    <rect x="4.5" y="5" width="15" height="10.5" rx="1.6" />
    <path d="M2.5 18.5h19" />
  {:else if nombre === 'captura'}
    <!-- Esquinas de una selección y una cruz al centro -->
    <path d="M4 8.5V6a2 2 0 0 1 2-2h2.5M15.5 4H18a2 2 0 0 1 2 2v2.5M20 15.5V18a2 2 0 0 1-2 2h-2.5M8.5 20H6a2 2 0 0 1-2-2v-2.5" />
    <path d="M9.5 12h5M12 9.5v5" />
  {:else if nombre === 'carpeta'}
    <path d="M3.5 7.2A1.7 1.7 0 0 1 5.2 5.5h3.9l2 2.2h7.7a1.7 1.7 0 0 1 1.7 1.7v8.4a1.7 1.7 0 0 1-1.7 1.7H5.2a1.7 1.7 0 0 1-1.7-1.7z" />
  {:else if nombre === 'portapapeles'}
    <rect x="5" y="4.5" width="14" height="16.5" rx="2" />
    <rect x="9" y="2.75" width="6" height="3.5" rx="1.2" />
    <path d="M8.5 11h7M8.5 15h5" />
  {:else if nombre === 'copiar'}
    <rect x="8.5" y="8.5" width="12" height="12" rx="2.2" />
    <path d="M15.5 8.5V5.7a2.2 2.2 0 0 0-2.2-2.2H5.7a2.2 2.2 0 0 0-2.2 2.2v7.6a2.2 2.2 0 0 0 2.2 2.2h2.8" />
  {:else if nombre === 'flecha'}
    <path d="M4.5 12h15M13.5 6l6 6-6 6" />
  {/if}
</svg>
