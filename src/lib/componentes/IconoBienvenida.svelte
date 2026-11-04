<!-- Íconos que usa la bienvenida y que Icono no tiene. Mismo juego: línea de 24×24, trazo 1.8, color del texto. -->
<script lang="ts" module>
  export type NombreBienvenida = 'persona' | 'accesibilidad' | 'notificaciones' | 'escudo' | 'barra-menus' | 'bandeja';
</script>

<script lang="ts">
  let { nombre, tamano = 18 }: { nombre: NombreBienvenida; tamano?: number } = $props();
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
  {#if nombre === 'persona'}
    <circle cx="12" cy="8" r="3.8" />
    <path d="M4.5 20.5a7.5 7.5 0 0 1 15 0" />
  {:else if nombre === 'accesibilidad'}
    <circle cx="12" cy="12" r="9" />
    <circle cx="12" cy="7.4" r="1.1" fill="currentColor" stroke="none" />
    <path d="M7.5 10.2l4.5 1 4.5-1" />
    <path d="M12 11.2v3.1" />
    <path d="M9.6 18l2.4-3.7 2.4 3.7" />
  {:else if nombre === 'notificaciones'}
    <path d="M6 16.5V11a6 6 0 0 1 12 0v5.5l1.5 2h-15z" />
    <path d="M10 21a2.2 2.2 0 0 0 4 0" />
  {:else if nombre === 'escudo'}
    <path d="M12 3l7.5 3v5.5c0 4.6-3.2 8.1-7.5 9.5-4.3-1.4-7.5-4.9-7.5-9.5V6z" />
    <path d="M8.8 12.2l2.3 2.3 4.1-4.3" />
  {:else if nombre === 'barra-menus'}
    <rect x="3" y="4.5" width="18" height="15" rx="2.5" />
    <path d="M3 8.5h18" />
    <path d="M16.5 6.5h.01M18.5 6.5h.01" />
  {:else if nombre === 'bandeja'}
    <rect x="3" y="4.5" width="18" height="15" rx="2.5" />
    <path d="M3 15.5h18" />
    <path d="M15.5 17.5h.01M18 17.5h.01" />
  {/if}
</svg>
