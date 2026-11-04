<!-- Deslizador (0–100, o 0–`maximo`): pista hundida, relleno con el color de su familia y perilla blanca.
     Con `pista`, la pista entera es ese fondo (el blanco cálido a frío o el arco iris de una luz) y no hay relleno.
     Funciona con teclado. -->
<script lang="ts">
  let {
    valor,
    etiqueta,
    color = 'celeste',
    apagado = false,
    tono,
    maximo = 100,
    minimo = 0,
    pista,
    deshabilitado = false,
    oncambio,
    ontocando,
  }: {
    valor: number;
    /** Hasta dónde llega (de fábrica, 100). */
    maximo?: number;
    /** Desde dónde (de fábrica, 0; el brillo de una luz empieza en 1). */
    minimo?: number;
    /** Fondo de la pista entera (un degradado); sin relleno. */
    pista?: string;
    deshabilitado?: boolean;
    etiqueta: string;
    /** celeste para sonido, amarillo para brillo, verde para batería y conexión, primario para lo demás */
    color?: 'celeste' | 'amarillo' | 'verde' | 'primario';
    apagado?: boolean;
    oncambio: (v: number) => void;
    ontocando?: (tocando: boolean) => void;
    /** Color del relleno a mano (por ejemplo, el tono del brillo); si no, el de la familia. */
    tono?: string;
  } = $props();
</script>

<input
  type="range"
  min={minimo}
  max={maximo}
  disabled={deshabilitado}
  step="1"
  value={valor}
  aria-label={etiqueta}
  class:apagado
  class:con-pista={!!pista}
  style:--pista={pista}
  style:--parte={Math.max(0, Math.min(1, (valor - minimo) / (maximo - minimo || 1)))}
  style:--relleno={tono ?? `var(--${color})`}
  oninput={(e) => oncambio(Number(e.currentTarget.value))}
  onpointerdown={(e) => {
    // Captura el puntero: aunque sueltes fuera de la barra, el "soltar" llega igual.
    e.currentTarget.setPointerCapture(e.pointerId);
    ontocando?.(true);
  }}
  onpointerup={() => ontocando?.(false)}
  onpointercancel={() => ontocando?.(false)}
  onchange={() => ontocando?.(false)}
/>

<style>
  input {
    --lleno: calc(10px + (100% - 20px) * var(--parte));
    -webkit-appearance: none;
    appearance: none;
    display: block;
    width: 100%;
    height: 24px;
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  /* El relleno llega al centro de la perilla (20 px): nunca asoma pista vacía debajo de ella. */
  input::-webkit-slider-runnable-track {
    height: 6px;
    border-radius: 99px;
    background:
      linear-gradient(var(--relleno), var(--relleno)) 0 0 / var(--lleno) 100% no-repeat,
      var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  input.con-pista::-webkit-slider-runnable-track {
    background: var(--pista);
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.12);
  }
  input:disabled {
    cursor: default;
    opacity: 0.45;
  }
  input.apagado::-webkit-slider-runnable-track {
    background:
      linear-gradient(var(--texto-3), var(--texto-3)) 0 0 / var(--lleno) 100% no-repeat,
      var(--superficie-hundida);
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 20px;
    height: 20px;
    margin-top: -7px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow:
      0 0 0 0.5px rgb(0 0 0 / 0.14),
      0 1px 2px rgb(0 0 0 / 0.2),
      0 3px 8px -2px rgb(0 0 0 / 0.28);
  }
  input:focus-visible {
    outline: none;
  }
  input:focus-visible::-webkit-slider-thumb {
    box-shadow: var(--foco);
  }
</style>
