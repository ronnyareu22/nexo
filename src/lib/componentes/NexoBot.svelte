<!--
  Nexo Bot, la mascota (DISENO.md, sección 6). Vector por capas: el cuerpo es el mismo en todas las
  expresiones; cambian los ojos, la boca, la luz del pecho y la pose de los brazos.
  El dibujo entre «generado:inicio» y «generado:fin» sale de diseno/mascota/generar.mjs: no se edita
  a mano. Para cambiarlo, edita el generador y corre `node diseno/mascota/generar.mjs`.
  `foto` muestra la ilustración original (solo para `saludo`).
-->
<script lang="ts" module>
  export type ExpresionNexoBot =
    | 'saludo'
    | 'feliz'
    | 'buscando'
    | 'desconectado'
    | 'error'
    | 'bluetooth-apagado'
    | 'cargando'
    | 'sin-dispositivos';

  const DESCRIPCION: Record<ExpresionNexoBot, string> = {
    saludo: 'Nexo Bot saluda',
    feliz: 'Nexo Bot contento',
    buscando: 'Nexo Bot buscando dispositivos',
    desconectado: 'Nexo Bot triste: el dispositivo se desconectó',
    error: 'Nexo Bot preocupado: error de conexión',
    'bluetooth-apagado': 'Nexo Bot dormido: el Bluetooth está apagado',
    cargando: 'Nexo Bot cargando',
    'sin-dispositivos': 'Nexo Bot curioso: no hay dispositivos',
  };
</script>

<script lang="ts">
  let {
    expresion = 'feliz',
    tamano = 160,
    etiqueta,
    foto = false,
  }: { expresion?: ExpresionNexoBot; tamano?: number; etiqueta?: string; foto?: boolean } = $props();

  // Ids propios por instancia: varios Nexo Bot en la misma vista no comparten degradados.
  const uid = $props.id();
  const p = `nb${uid}`;
  const nombre = $derived(etiqueta ?? DESCRIPCION[expresion]);
  const conFoto = $derived(foto && expresion === 'saludo');
</script>

{#if conFoto}
  <img class="nexo-bot" src="/mascota/saludo.webp" width={tamano} height={tamano} alt={nombre} decoding="async" draggable="false" />
{:else}
  <svg class="nexo-bot" viewBox="0 0 240 240" width={tamano} height={tamano} role="img" aria-label={nombre}>
    <!-- generado:inicio -->
<defs><linearGradient id="{p}b" x1="0" y1="0" x2=".75" y2="1"><stop offset="0" stop-color="#fff"/><stop offset=".45" stop-color="#F4F6FA"/><stop offset="1" stop-color="#DDE3EC"/></linearGradient><linearGradient id="{p}o" x1="0" y1="0" x2=".6" y2="1"><stop offset="0" stop-color="#4A525F"/><stop offset=".5" stop-color="#2B313B"/><stop offset="1" stop-color="#1A1E25"/></linearGradient><radialGradient id="{p}s" cx=".32" cy=".22" r=".85"><stop offset="0" stop-color="#1A2230"/><stop offset=".6" stop-color="#05070B"/></radialGradient><radialGradient id="{p}g"><stop offset="0" stop-color="#39D5FF" stop-opacity=".55"/><stop offset="1" stop-color="#39D5FF" stop-opacity="0"/></radialGradient><radialGradient id="{p}a"><stop offset="0" stop-color="#F59E0B" stop-opacity=".5"/><stop offset="1" stop-color="#F59E0B" stop-opacity="0"/></radialGradient><radialGradient id="{p}h"><stop offset="0" stop-color="#0B1220" stop-opacity=".2"/><stop offset="1" stop-color="#0B1220" stop-opacity="0"/></radialGradient></defs>
<g transform="matrix(.92 0 0 .92 13 12)">
<g class="cuerpo"><ellipse cx="124" cy="229" rx="70" ry="7" fill="url(#{p}h)"/><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M89 166C100 162 114 168 118 178L115 208H65C69 190 77 174 89 166Z"/><path d="M134 176C142 170 160 170 167 177C171 186 174 197 175 208H129C129 196 130 185 134 176Z"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M83.5 178.5L74 198.5" stroke-width="6.2" opacity=".1"/><path d="M83.5 178.5L74 198.5" stroke-width="4.1" opacity=".2"/><path d="M83.5 178.5L74 198.5" stroke-width="2.4"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M168.5 183L171.8 196.5" stroke-width="6.2" opacity=".1"/><path d="M168.5 183L171.8 196.5" stroke-width="4.1" opacity=".2"/><path d="M168.5 183L171.8 196.5" stroke-width="2.4"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M58 223C58 208 72 200 88 200C105 200 117 208 117 221Z"/><path d="M123 220C123 206 138 198 157 198C177 198 192 206 192 219Z"/></g><g fill="url(#{p}o)"><rect x="56" y="218" width="63" height="13" rx="6.5"/><rect x="121" y="215" width="73" height="13" rx="6.5"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M106.5 213.5L108.3 220.5" stroke-width="7.8" opacity=".1"/><path d="M106.5 213.5L108.3 220.5" stroke-width="5.1" opacity=".2"/><path d="M106.5 213.5L108.3 220.5" stroke-width="3"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M148 212.5L150.3 219.5" stroke-width="7.8" opacity=".1"/><path d="M148 212.5L150.3 219.5" stroke-width="5.1" opacity=".2"/><path d="M148 212.5L150.3 219.5" stroke-width="3"/></g><g fill="url(#{p}o)"><ellipse cx="101" cy="175" rx="16" ry="9" transform="rotate(32 101 175)"/><ellipse cx="143" cy="180" rx="13" ry="8.5" transform="rotate(-38 143 180)"/><ellipse cx="128" cy="118" rx="19" ry="7"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M120 116C142 116 156 132 156 153C156 174 141 188 120 188C99 188 85 174 85 152C85 131 99 116 120 116Z"/></g></g>

{#if expresion === 'saludo' || expresion === 'feliz' || expresion === 'buscando' || expresion === 'cargando' || expresion === 'sin-dispositivos'}
<g class="pecho"><ellipse cx="131" cy="143" rx="21" ry="21" fill="url(#{p}g)"/><ellipse cx="131" cy="143" rx="12.5" ry="12.5" fill="#0B1017" stroke="#3A424E" stroke-width="1"/><circle cx="131" cy="143" r="10.6" fill="none" stroke="#39D5FF" stroke-width="1.8"/><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138" stroke-width="5.7" opacity=".1"/><path d="M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138" stroke-width="3.7" opacity=".2"/><path d="M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138" stroke-width="2.2"/></g></g>
{:else if expresion === 'desconectado'}
<g class="pecho"><ellipse cx="131" cy="143" rx="12.5" ry="12.5" fill="#0B1017" stroke="#3A424E" stroke-width="1"/><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round" opacity=".38"><circle cx="131" cy="143" r="10.6" stroke-width="1.8"/><path d="M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138" stroke-width="2.4"/></g></g>
{:else if expresion === 'error'}
<g class="pecho"><ellipse cx="131" cy="143" rx="21" ry="21" fill="url(#{p}a)"/><ellipse cx="131" cy="143" rx="12.5" ry="12.5" fill="#0B1017" stroke="#3A424E" stroke-width="1"/><circle cx="131" cy="143" r="10.6" fill="none" stroke="#F59E0B" stroke-width="1.8"/><g fill="none" stroke="#F59E0B" stroke-linecap="round" stroke-linejoin="round"><path d="M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138" stroke-width="5.7" opacity=".1"/><path d="M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138" stroke-width="3.7" opacity=".2"/><path d="M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138" stroke-width="2.2"/></g></g>
{:else}
<g class="pecho"><ellipse cx="131" cy="143" rx="12.5" ry="12.5" fill="#0B1017" stroke="#3A424E" stroke-width="1"/><g fill="none" stroke="#6B7686" stroke-linecap="round" stroke-linejoin="round"><circle cx="131" cy="143" r="10.6" stroke-width="1.8"/><path d="M124 148V141.5C124 137 128.5 136.5 130.2 140L132 144C133.7 147.6 138 147 138 142.5V138" stroke-width="2.4"/></g></g>
{/if}
<g class="brazos">
{#if expresion === 'saludo'}
<g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M46.1 133.4L75.1 142.4A13 13 0 0 0 82.9 117.6L53.9 108.6A13 13 0 0 0 46.1 133.4Z"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M41.8 121.5Q43 130 52.5 131.5" stroke-width="6.2" opacity=".1"/><path d="M41.8 121.5Q43 130 52.5 131.5" stroke-width="4.1" opacity=".2"/><path d="M41.8 121.5Q43 130 52.5 131.5" stroke-width="2.4"/></g><g fill="url(#{p}o)"><ellipse cx="81" cy="130" rx="6" ry="15" transform="rotate(15 81 130)"/><path d="M33.2 92.8L24.2 80.8A5.3 5.3 0 0 0 15.8 87.2L24.8 99.2A5.3 5.3 0 0 0 33.2 92.8Z"/><path d="M38.5 91.1L35.5 73.1A5.6 5.6 0 0 0 24.5 74.9L27.5 92.9A5.6 5.6 0 0 0 38.5 91.1Z"/><path d="M51.6 90L51.6 72A5.6 5.6 0 0 0 40.4 72L40.4 90A5.6 5.6 0 0 0 51.6 90Z"/><path d="M60.1 101.3L66.1 88.3A5.6 5.6 0 0 0 55.9 83.7L49.9 96.7A5.6 5.6 0 0 0 60.1 101.3Z"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M30.7 89.3L24.2 80.8A5.3 5.3 0 0 0 15.8 87.2L22.3 95.7A5.3 5.3 0 0 0 30.7 89.3Z"/><path d="M37.2 83.6L35.5 73.1A5.6 5.6 0 0 0 24.5 74.9L26.2 85.4A5.6 5.6 0 0 0 37.2 83.6Z"/><path d="M51.6 83L51.6 72A5.6 5.6 0 0 0 40.4 72L40.4 83A5.6 5.6 0 0 0 51.6 83Z"/><path d="M63 95.7L66.2 88.2A5.6 5.6 0 0 0 55.8 83.8L52.6 91.3A5.6 5.6 0 0 0 63 95.7Z"/><rect x="27" y="87" width="33" height="24" rx="11" transform="rotate(-8 43.5 99)"/></g><g fill="url(#{p}o)"><ellipse cx="47.5" cy="110.5" rx="12.5" ry="4.6" transform="rotate(-6 47.5 110.5)"/></g><g fill="url(#{p}o)"><ellipse cx="158" cy="137" rx="5.5" ry="14" transform="rotate(-12 158 137)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M154.8 151.8L189.8 168.8A12 12 0 0 0 200.2 147.2L165.2 130.2A12 12 0 0 0 154.8 151.8Z"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M200.4 150.2Q211.6 158 200.4 165.8" stroke-width="5.7" opacity=".1"/><path d="M200.4 150.2Q211.6 158 200.4 165.8" stroke-width="3.7" opacity=".2"/><path d="M200.4 150.2Q211.6 158 200.4 165.8" stroke-width="2.2"/></g><g fill="url(#{p}o)"><ellipse cx="189" cy="156" rx="6" ry="15.5" transform="rotate(-8 189 156)"/><path d="M185.8 141L185.8 135A6.8 6.8 0 0 0 172.2 135L172.2 141A6.8 6.8 0 0 0 185.8 141Z"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M185.2 136L185.2 127A6.2 6.2 0 0 0 172.8 127L172.8 136A6.2 6.2 0 0 0 185.2 136Z"/><rect x="162" y="138" width="29" height="33" rx="12" transform="rotate(-6 176.5 154.5)"/></g><path d="M165 148H175M165 155.5H176M165 163H175" fill="none" stroke="#C3CBD8" stroke-width="1.3" stroke-linecap="round"/>
{:else if expresion === 'feliz'}
<g fill="url(#{p}o)"><ellipse cx="82" cy="133" rx="5.5" ry="14" transform="rotate(12 82 133)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M74.8 126.2L39.8 143.2A12 12 0 0 0 50.2 164.8L85.2 147.8A12 12 0 0 0 74.8 126.2Z"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M39.6 146.2Q28.4 154 39.6 161.8" stroke-width="5.7" opacity=".1"/><path d="M39.6 146.2Q28.4 154 39.6 161.8" stroke-width="3.7" opacity=".2"/><path d="M39.6 146.2Q28.4 154 39.6 161.8" stroke-width="2.2"/></g><g fill="url(#{p}o)"><ellipse cx="51" cy="152" rx="6" ry="15.5" transform="rotate(8 51 152)"/><path d="M67.8 137L67.8 131A6.8 6.8 0 0 0 54.2 131L54.2 137A6.8 6.8 0 0 0 67.8 137Z"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M67.2 132L67.2 123A6.2 6.2 0 0 0 54.8 123L54.8 132A6.2 6.2 0 0 0 67.2 132Z"/><rect x="49" y="134" width="29" height="33" rx="12" transform="rotate(6 63.5 150.5)"/></g><path d="M75 144H65M75 151.5H64M75 159H65" fill="none" stroke="#C3CBD8" stroke-width="1.3" stroke-linecap="round"/><g fill="url(#{p}o)"><ellipse cx="158" cy="137" rx="5.5" ry="14" transform="rotate(-12 158 137)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M154.8 151.8L189.8 168.8A12 12 0 0 0 200.2 147.2L165.2 130.2A12 12 0 0 0 154.8 151.8Z"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M200.4 150.2Q211.6 158 200.4 165.8" stroke-width="5.7" opacity=".1"/><path d="M200.4 150.2Q211.6 158 200.4 165.8" stroke-width="3.7" opacity=".2"/><path d="M200.4 150.2Q211.6 158 200.4 165.8" stroke-width="2.2"/></g><g fill="url(#{p}o)"><ellipse cx="189" cy="156" rx="6" ry="15.5" transform="rotate(-8 189 156)"/><path d="M185.8 141L185.8 135A6.8 6.8 0 0 0 172.2 135L172.2 141A6.8 6.8 0 0 0 185.8 141Z"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M185.2 136L185.2 127A6.2 6.2 0 0 0 172.8 127L172.8 136A6.2 6.2 0 0 0 185.2 136Z"/><rect x="162" y="138" width="29" height="33" rx="12" transform="rotate(-6 176.5 154.5)"/></g><path d="M165 148H175M165 155.5H176M165 163H175" fill="none" stroke="#C3CBD8" stroke-width="1.3" stroke-linecap="round"/>
{:else if expresion === 'sin-dispositivos'}
<g fill="url(#{p}o)"><ellipse cx="83" cy="133" rx="5.5" ry="13" transform="rotate(25 83 133)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M76 128.3L55 140.3A10 10 0 0 0 65 157.7L86 145.7A10 10 0 0 0 76 128.3Z"/><path d="M67.8 144.4L57.8 127.4A9 9 0 0 0 42.2 136.6L52.2 153.6A9 9 0 0 0 67.8 144.4Z"/></g><g fill="url(#{p}o)"><ellipse cx="60" cy="149" rx="5" ry="9.5" transform="rotate(30 60 149)"/><ellipse cx="49.5" cy="130" rx="7.5" ry="3.2" transform="rotate(-32 49.5 130)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M54 117.8L56.5 108.8A3.1 3.1 0 0 0 50.5 107.2L48 116.2A3.1 3.1 0 0 0 54 117.8Z"/><path d="M49.1 115.8L48.6 105.8A3.1 3.1 0 0 0 42.4 106.2L42.9 116.2A3.1 3.1 0 0 0 49.1 115.8Z"/><path d="M44 116.1L41.5 107.6A3.1 3.1 0 0 0 35.5 109.4L38 117.9A3.1 3.1 0 0 0 44 116.1Z"/><path d="M55.2 126.4L60.7 121.4A3.2 3.2 0 0 0 56.3 116.6L50.8 121.6A3.2 3.2 0 0 0 55.2 126.4Z"/><rect x="39" y="114" width="15" height="14" rx="6" transform="rotate(-12 46.5 121)"/></g><g fill="url(#{p}o)"><ellipse cx="157" cy="137" rx="5.5" ry="13" transform="rotate(-25 157 137)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M154 149.7L175 161.7A10 10 0 0 0 185 144.3L164 132.3A10 10 0 0 0 154 149.7Z"/><path d="M187.8 157.6L197.8 140.6A9 9 0 0 0 182.2 131.4L172.2 148.4A9 9 0 0 0 187.8 157.6Z"/></g><g fill="url(#{p}o)"><ellipse cx="180" cy="153" rx="5" ry="9.5" transform="rotate(-30 180 153)"/><ellipse cx="190.5" cy="134" rx="7.5" ry="3.2" transform="rotate(32 190.5 134)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M192 120.2L189.5 111.2A3.1 3.1 0 0 0 183.5 112.8L186 121.8A3.1 3.1 0 0 0 192 120.2Z"/><path d="M197.1 120.2L197.6 110.2A3.1 3.1 0 0 0 191.4 109.8L190.9 119.8A3.1 3.1 0 0 0 197.1 120.2Z"/><path d="M202 121.9L204.5 113.4A3.1 3.1 0 0 0 198.5 111.6L196 120.1A3.1 3.1 0 0 0 202 121.9Z"/><path d="M189.2 125.6L183.7 120.6A3.2 3.2 0 0 0 179.3 125.4L184.8 130.4A3.2 3.2 0 0 0 189.2 125.6Z"/><rect x="186" y="118" width="15" height="14" rx="6" transform="rotate(12 193.5 125)"/></g>
{:else}
<g fill="url(#{p}o)"><ellipse cx="83" cy="133" rx="5.5" ry="13" transform="rotate(25 83 133)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M72.4 131.7L63.4 151.7A10.5 10.5 0 0 0 82.6 160.3L91.6 140.3A10.5 10.5 0 0 0 72.4 131.7Z"/></g><g fill="url(#{p}o)"><ellipse cx="72" cy="159" rx="9.5" ry="4" transform="rotate(24 72 159)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><ellipse cx="70.5" cy="167.5" rx="8" ry="9.5" transform="rotate(22 70.5 167.5)"/></g><g fill="url(#{p}o)"><ellipse cx="157" cy="137" rx="5.5" ry="13" transform="rotate(-25 157 137)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><path d="M148.4 144.3L157.4 164.3A10.5 10.5 0 0 0 176.6 155.7L167.6 135.7A10.5 10.5 0 0 0 148.4 144.3Z"/></g><g fill="url(#{p}o)"><ellipse cx="168" cy="163" rx="9.5" ry="4" transform="rotate(-24 168 163)"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><ellipse cx="169.5" cy="171.5" rx="8" ry="9.5" transform="rotate(-22 169.5 171.5)"/></g>
{/if}
</g>
<g class="cabeza" transform="rotate(8 131 72)">
<path d="M79 46C80 3 197 1 200 48" fill="none" stroke="url(#{p}b)" stroke-width="19"/><path d="M79 46C80 3 197 1 200 48" fill="none" stroke="url(#{p}o)" stroke-width="12.5"/><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M105 18.6C126 11.2 155.2 11.4 175.7 19.9" stroke-width="9.4" opacity=".1"/><path d="M105 18.6C126 11.2 155.2 11.4 175.7 19.9" stroke-width="6.1" opacity=".2"/><path d="M105 18.6C126 11.2 155.2 11.4 175.7 19.9" stroke-width="3.6"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><ellipse cx="67" cy="64" rx="11" ry="23"/><rect x="72" y="34" width="13" height="15" rx="4" transform="rotate(-24 78.5 41.5)"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M64.5 44A8 20 0 0 0 64.5 84" stroke-width="7.3" opacity=".1"/><path d="M64.5 44A8 20 0 0 0 64.5 84" stroke-width="4.8" opacity=".2"/><path d="M64.5 44A8 20 0 0 0 64.5 84" stroke-width="2.8"/></g><g fill="url(#{p}o)"><ellipse cx="77.5" cy="64" rx="5.5" ry="22.5"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><rect x="75" y="28" width="112" height="90" rx="38"/></g><rect x="83" y="40" width="96" height="68" rx="26" fill="url(#{p}s)" stroke="#1E242D" stroke-width="2"/><path d="M98 50.5Q131 41.5 164 50.5" fill="none" stroke="#fff" opacity=".09" stroke-width="5" stroke-linecap="round"/>
<g class="cara">
{#if expresion === 'saludo' || expresion === 'feliz'}
<g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M97 75A11 11 0 0 1 119 75M144 75A11 11 0 0 1 166 75" stroke-width="13" opacity=".1"/><path d="M97 75A11 11 0 0 1 119 75M144 75A11 11 0 0 1 166 75" stroke-width="8.5" opacity=".2"/><path d="M97 75A11 11 0 0 1 119 75M144 75A11 11 0 0 1 166 75" stroke-width="5"/></g><path d="M88 81l2.4-3.4M91.6 82l2.4-3.4M166 81l2.4-3.4M169.6 82l2.4-3.4" fill="none" stroke="#39D5FF" stroke-opacity=".75" stroke-width="1.8" stroke-linecap="round"/><path d="M123.5 82H139.5A8 9 0 0 1 123.5 82Z" fill="none" stroke="#39D5FF" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="M123.5 82H139.5A8 9 0 0 1 123.5 82Z" fill="#39D5FF"/>
{:else if expresion === 'buscando'}
<g class="buscar"><path d="M107 72a6 6 0 1 0 12 0a6 6 0 1 0 -12 0ZM154 72a6 6 0 1 0 12 0a6 6 0 1 0 -12 0Z" fill="none" stroke="#39D5FF" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="M107 72a6 6 0 1 0 12 0a6 6 0 1 0 -12 0ZM154 72a6 6 0 1 0 12 0a6 6 0 1 0 -12 0Z" fill="#39D5FF"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M127.5 88Q132 90 136.5 88" stroke-width="6.8" opacity=".1"/><path d="M127.5 88Q132 90 136.5 88" stroke-width="4.4" opacity=".2"/><path d="M127.5 88Q132 90 136.5 88" stroke-width="2.6"/></g>
{:else if expresion === 'desconectado'}
<path d="M100.9 70.4L113.6 67.9A7.6 7.6 0 1 1 100.9 70.4ZM162.1 70.4L149.4 67.9A7.6 7.6 0 1 0 162.1 70.4Z" fill="none" stroke="#39D5FF" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="M100.9 70.4L113.6 67.9A7.6 7.6 0 1 1 100.9 70.4ZM162.1 70.4L149.4 67.9A7.6 7.6 0 1 0 162.1 70.4Z" fill="#39D5FF"/><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M126.5 90Q131.5 88 136.5 90" stroke-width="6.8" opacity=".1"/><path d="M126.5 90Q131.5 88 136.5 90" stroke-width="4.4" opacity=".2"/><path d="M126.5 90Q131.5 88 136.5 90" stroke-width="2.6"/></g>
{:else if expresion === 'error'}
<path d="M102 73a6 6 0 1 0 12 0a6 6 0 1 0 -12 0ZM149 73a6 6 0 1 0 12 0a6 6 0 1 0 -12 0Z" fill="none" stroke="#39D5FF" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="M102 73a6 6 0 1 0 12 0a6 6 0 1 0 -12 0ZM149 73a6 6 0 1 0 12 0a6 6 0 1 0 -12 0Z" fill="#39D5FF"/><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M100 65L114 60.5M163 65L149 60.5" stroke-width="6.8" opacity=".1"/><path d="M100 65L114 60.5M163 65L149 60.5" stroke-width="4.4" opacity=".2"/><path d="M100 65L114 60.5M163 65L149 60.5" stroke-width="2.6"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M127.9 89a3.6 3.6 0 1 0 7.2 0a3.6 3.6 0 1 0 -7.2 0Z" stroke-width="6.2" opacity=".1"/><path d="M127.9 89a3.6 3.6 0 1 0 7.2 0a3.6 3.6 0 1 0 -7.2 0Z" stroke-width="4.1" opacity=".2"/><path d="M127.9 89a3.6 3.6 0 1 0 7.2 0a3.6 3.6 0 1 0 -7.2 0Z" stroke-width="2.4"/></g>
{:else if expresion === 'bluetooth-apagado'}
<g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M100 74H116M147 74H163" stroke-width="9.4" opacity=".1"/><path d="M100 74H116M147 74H163" stroke-width="6.1" opacity=".2"/><path d="M100 74H116M147 74H163" stroke-width="3.6"/></g><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M128 88Q131.5 89.6 135 88" stroke-width="6.8" opacity=".1"/><path d="M128 88Q131.5 89.6 135 88" stroke-width="4.4" opacity=".2"/><path d="M128 88Q131.5 89.6 135 88" stroke-width="2.6"/></g>
{:else if expresion === 'cargando'}
<g class="punto"><path d="M111.9 74a4.6 4.6 0 1 0 9.2 0a4.6 4.6 0 1 0 -9.2 0Z" fill="none" stroke="#39D5FF" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="M111.9 74a4.6 4.6 0 1 0 9.2 0a4.6 4.6 0 1 0 -9.2 0Z" fill="#39D5FF"/></g><g class="punto" opacity=".6"><path d="M126.9 74a4.6 4.6 0 1 0 9.2 0a4.6 4.6 0 1 0 -9.2 0Z" fill="none" stroke="#39D5FF" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="M126.9 74a4.6 4.6 0 1 0 9.2 0a4.6 4.6 0 1 0 -9.2 0Z" fill="#39D5FF"/></g><g class="punto" opacity=".3"><path d="M141.9 74a4.6 4.6 0 1 0 9.2 0a4.6 4.6 0 1 0 -9.2 0Z" fill="none" stroke="#39D5FF" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="M141.9 74a4.6 4.6 0 1 0 9.2 0a4.6 4.6 0 1 0 -9.2 0Z" fill="#39D5FF"/></g>
{:else}
<path d="M100 72a8 8 0 1 0 16 0a8 8 0 1 0 -16 0ZM150.2 74a4.8 4.8 0 1 0 9.6 0a4.8 4.8 0 1 0 -9.6 0Z" fill="none" stroke="#39D5FF" stroke-width="4" opacity=".2" stroke-linejoin="round"/><path d="M100 72a8 8 0 1 0 16 0a8 8 0 1 0 -16 0ZM150.2 74a4.8 4.8 0 1 0 9.6 0a4.8 4.8 0 1 0 -9.6 0Z" fill="#39D5FF"/><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-linejoin="round"><path d="M126.5 89.5Q131.5 90.5 136.5 87.5" stroke-width="6.8" opacity=".1"/><path d="M126.5 89.5Q131.5 90.5 136.5 87.5" stroke-width="4.4" opacity=".2"/><path d="M126.5 89.5Q131.5 90.5 136.5 87.5" stroke-width="2.6"/></g>
{/if}
</g>
<g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><rect x="193" y="37" width="13" height="16" rx="4" transform="rotate(24 199.5 45)"/></g><g fill="url(#{p}o)"><ellipse cx="191" cy="76" rx="8" ry="28"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><ellipse cx="204" cy="76" rx="14" ry="30"/><ellipse cx="209" cy="76" rx="9" ry="26"/></g><g fill="none" stroke="#39D5FF"><ellipse cx="210" cy="76" rx="6.6" ry="21.5" stroke-width="7" opacity=".2"/><ellipse cx="210" cy="76" rx="6.6" ry="21.5" stroke-width="3.2"/></g><g fill="url(#{p}b)" stroke="#C9D1DD" stroke-width=".9"><ellipse cx="211" cy="76" rx="4" ry="17.5"/></g>
</g>
{#if expresion === 'buscando'}
<g class="extra"><g fill="none" stroke="#39D5FF" stroke-width="2.6" stroke-linecap="round"><path d="M208.9 36.2A9 9 0 0 1 215.8 43.1" opacity="0.9"/><path d="M210.3 29.3A16 16 0 0 1 222.7 41.7" opacity="0.55"/><path d="M211.8 22.5A23 23 0 0 1 229.5 40.2" opacity="0.3"/></g></g>
{:else if expresion === 'bluetooth-apagado'}
<g class="extra"><g fill="none" stroke="#8C98AA" stroke-linecap="round" stroke-linejoin="round"><path d="M209 30h6l-6 7h6" stroke-width="2"/><path d="M219 11h9l-9 10.5h9" stroke-width="2.6"/></g></g>
{:else if expresion === 'sin-dispositivos'}
<g class="extra"><g fill="none" stroke="#39D5FF" stroke-linecap="round" stroke-width="3"><path d="M206 25.5q0-7.5 7.5-7.5t7.5 7q0 4.5-7 7v3.5"/></g><circle cx="214" cy="42.5" r="2" fill="#39D5FF"/></g>
{/if}
</g>
<!-- generado:fin -->
  </svg>
{/if}

<style>
  .nexo-bot {
    display: block;
    flex-shrink: 0;
    animation: entrada 300ms cubic-bezier(0.2, 0.7, 0.2, 1) both;
  }
  img.nexo-bot {
    object-fit: contain;
    user-select: none;
  }
  @keyframes entrada {
    from {
      opacity: 0;
      transform: translateY(6px);
    }
  }

  /* Bucles: solo los que son indicadores de carga (buscar y cargar). */
  .buscar {
    animation: mirar 2.4s ease-in-out infinite;
  }
  @keyframes mirar {
    50% {
      transform: translateX(-9px);
    }
  }
  .punto {
    animation: pulso 1.2s ease-in-out infinite;
    transform-box: fill-box;
    transform-origin: center;
  }
  .punto + .punto {
    animation-delay: 0.2s;
  }
  .punto + .punto + .punto {
    animation-delay: 0.4s;
  }
  @keyframes pulso {
    0%,
    100% {
      opacity: 0.3;
      transform: scale(0.8);
    }
    40% {
      opacity: 1;
      transform: none;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .nexo-bot,
    .buscar,
    .punto {
      animation: none;
    }
  }
</style>
