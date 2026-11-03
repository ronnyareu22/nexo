<!-- Deslizador 0–100: pista hundida, relleno con el color de su familia y perilla blanca. Funciona con teclado. -->
<script lang="ts">
  let {
    valor,
    etiqueta,
    color = 'celeste',
    apagado = false,
    oncambio,
    ontocando,
  }: {
    valor: number;
    etiqueta: string;
    /** celeste para sonido, amarillo para brillo, verde para batería y conexión, primario para lo demás */
    color?: 'celeste' | 'amarillo' | 'verde' | 'primario';
    apagado?: boolean;
    oncambio: (v: number) => void;
    ontocando?: (tocando: boolean) => void;
  } = $props();
</script>

<input
  type="range"
  min="0"
  max="100"
  step="1"
  value={valor}
  aria-label={etiqueta}
  class:apagado
  style:--lleno="{valor}%"
  style:--relleno="var(--{color})"
  oninput={(e) => oncambio(Number(e.currentTarget.value))}
  onpointerdown={() => ontocando?.(true)}
  onpointerup={() => ontocando?.(false)}
  onpointercancel={() => ontocando?.(false)}
/>

<style>
  input {
    -webkit-appearance: none;
    appearance: none;
    width: 100%;
    height: 26px;
    margin: 0;
    background: transparent;
    cursor: pointer;
  }
  input::-webkit-slider-runnable-track {
    height: 8px;
    border-radius: 99px;
    background:
      linear-gradient(var(--relleno), var(--relleno)) 0 0 / var(--lleno) 100% no-repeat,
      var(--superficie-hundida);
    box-shadow: inset 0 0 0 1px var(--borde);
  }
  input.apagado::-webkit-slider-runnable-track {
    background:
      linear-gradient(var(--texto-3), var(--texto-3)) 0 0 / var(--lleno) 100% no-repeat,
      var(--superficie-hundida);
  }
  input::-webkit-slider-thumb {
    -webkit-appearance: none;
    width: 22px;
    height: 22px;
    margin-top: -7px;
    border-radius: 50%;
    background: #ffffff;
    box-shadow:
      0 1px 4px rgb(0 0 0 / 0.3),
      0 0 0 0.5px rgb(0 0 0 / 0.08);
  }
  input:focus-visible {
    outline: none;
  }
  input:focus-visible::-webkit-slider-thumb {
    box-shadow: var(--foco);
  }
</style>
