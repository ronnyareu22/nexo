// Widgets nativos de Nexo para macOS (WidgetKit), los mismos que se ponen junto al Calendario o el Clima.
// No usan memoria de Nexo: los dibuja el sistema. Leen el estado que Nexo escribe cuando algo cambia:
//   ~/Library/Application Support/com.ronny.nexo/estado-widget.json
// Se compila sin Xcode con widget-mac/construir.sh.
import AppIntents
import SwiftUI
import WidgetKit
import notify

// MARK: - Estado que escribe Nexo

struct EstadoNexo: Decodable {
  struct Aparato: Decodable, Identifiable {
    let id: String
    let nombre: String
    let tipo: String
    let conectado: Bool
    let bateria: Int?
    let vendor: String?
    let producto: String?
  }
  struct Pantalla: Decodable {
    let nombre: String
    let interna: Bool
    let brillo: Int?
  }
  let actualizado: Double
  let bluetooth: Bool
  let dispositivos: [Aparato]
  let pantallas: [Pantalla]

  /// Los audífonos conectados (o, si no hay, los primeros emparejados).
  var audifonos: Aparato? {
    let deSonido = dispositivos.filter { ["audifonos", "diadema", "parlante"].contains($0.tipo) }
    return deSonido.first { $0.conectado } ?? deSonido.first
  }
  var conectados: Int { dispositivos.filter(\.conectado).count }

  static let ejemplo = EstadoNexo(
    actualizado: Date().timeIntervalSince1970, bluetooth: true,
    dispositivos: [
      .init(id: "1", nombre: "WH-CH520", tipo: "diadema", conectado: true, bateria: 100, vendor: "0x054C", producto: "0x0EAD"),
      .init(id: "2", nombre: "Beats Solo³", tipo: "audifonos", conectado: false, bateria: nil, vendor: "0x004C", producto: "0x2006"),
    ],
    pantallas: [.init(nombre: "Pantalla de la MacBook", interna: true, brillo: 31)])
}

/// Dentro del sandbox el "home" es la carpeta del widget; el archivo está en el home real del usuario.
func rutaEstado() -> URL {
  let home = getpwuid(getuid()).map { String(cString: $0.pointee.pw_dir) } ?? NSHomeDirectory()
  return URL(fileURLWithPath: home)
    .appendingPathComponent("Library/Application Support/com.ronny.nexo/estado-widget.json")
}

func leerEstado() -> EstadoNexo? {
  guard let datos = try? Data(contentsOf: rutaEstado()) else { return nil }
  return try? JSONDecoder().decode(EstadoNexo.self, from: datos)
}

// MARK: - Línea de tiempo

struct Entrada: TimelineEntry {
  let date: Date
  let estado: EstadoNexo?
}

struct Proveedor: TimelineProvider {
  func placeholder(in context: Context) -> Entrada { Entrada(date: .now, estado: .ejemplo) }

  func getSnapshot(in context: Context, completion: @escaping (Entrada) -> Void) {
    // En la galería de widgets se muestra el estado real si hay; si no, el ejemplo.
    completion(Entrada(date: .now, estado: leerEstado() ?? (context.isPreview ? .ejemplo : nil)))
  }

  func getTimeline(in context: Context, completion: @escaping (Timeline<Entrada>) -> Void) {
    // Nexo pide recargar cuando algo cambia; esto es solo por si acaso.
    let siguiente = Date.now.addingTimeInterval(10 * 60)
    completion(Timeline(entries: [Entrada(date: .now, estado: leerEstado())], policy: .after(siguiente)))
  }
}

// MARK: - Colores de Nexo

extension Color {
  static let nexoTurquesa = Color(red: 0, green: 0.90, blue: 0.72)
  static let nexoVerde = Color(red: 0.13, green: 0.77, blue: 0.37)
  static let nexoCeleste = Color(red: 0.22, green: 0.74, blue: 0.97)
  static let nexoAmbar = Color(red: 0.96, green: 0.62, blue: 0.04)
  static let nexoRojo = Color(red: 0.94, green: 0.27, blue: 0.27)
}

func colorBateria(_ nivel: Int) -> Color {
  nivel <= 15 ? .nexoRojo : nivel < 40 ? .nexoAmbar : .nexoVerde
}

/// Foto del modelo, guardada dentro del widget (la clave es "marca:modelo" del Bluetooth).
func fotoDe(_ a: EstadoNexo.Aparato) -> NSImage? {
  let fotos = ["0x004C:0x2006": "beats-solo3", "0x054C:0x0EAD": "sony-wh-ch520"]
  guard let v = a.vendor, let p = a.producto, let nombre = fotos["\(v.uppercased().replacingOccurrences(of: "0X", with: "0x")):\(p.uppercased().replacingOccurrences(of: "0X", with: "0x"))"],
    let url = Bundle.main.url(forResource: nombre, withExtension: "png")
  else { return nil }
  return NSImage(contentsOf: url)
}

/// Fondo de los widgets: el del sistema con un resplandor turquesa quieto arriba a la derecha (la aurora de Nexo).
struct FondoNexo: View {
  var body: some View {
    ZStack {
      Color(nsColor: .windowBackgroundColor)
      RadialGradient(
        colors: [Color.nexoTurquesa.opacity(0.22), Color.blue.opacity(0.10), .clear],
        center: .topTrailing, startRadius: 0, endRadius: 190)
    }
  }
}

struct Anillo: View {
  let nivel: Int
  var grosor: CGFloat = 6
  var body: some View {
    ZStack {
      Circle().stroke(Color.primary.opacity(0.12), lineWidth: grosor)
      Circle()
        .trim(from: 0, to: CGFloat(nivel) / 100)
        .stroke(colorBateria(nivel), style: StrokeStyle(lineWidth: grosor, lineCap: .round))
        .rotationEffect(.degrees(-90))
      Text("\(nivel)%")
        .font(.system(size: 15, weight: .bold, design: .rounded))
        .monospacedDigit()
    }
  }
}

// MARK: - Widget pequeño: tus audífonos

struct VistaAudifonos: View {
  let estado: EstadoNexo?

  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Image(systemName: "headphones")
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(Color.nexoCeleste)
        .accessibilityLabel("Audífonos")

      Spacer(minLength: 4)

      if let a = estado?.audifonos {
        HStack(alignment: .center, spacing: 10) {
          if a.conectado, let nivel = a.bateria {
            Anillo(nivel: nivel).frame(width: 58, height: 58)
          } else if let foto = fotoDe(a) {
            Image(nsImage: foto).resizable().scaledToFit().frame(width: 58, height: 58)
              .opacity(a.conectado ? 1 : 0.55)
          }
          Spacer(minLength: 0)
        }
        Spacer(minLength: 6)
        Text(a.nombre)
          .font(.system(size: 14, weight: .semibold))
          .lineLimit(2)
          .minimumScaleFactor(0.8)
        HStack(spacing: 5) {
          Circle().fill(a.conectado ? Color.nexoVerde : Color.secondary).frame(width: 7, height: 7)
          Text(a.conectado ? "Conectados" : "Desconectados")
        }
        .font(.system(size: 12, weight: .medium))
        .foregroundStyle(a.conectado ? Color.nexoVerde : Color.secondary)
      } else {
        Text(estado == nil ? "Abre Nexo" : "Sin audífonos")
          .font(.system(size: 17, weight: .bold))
        Text(estado == nil ? "para ver tus aparatos aquí." : "Empareja unos en Ajustes › Bluetooth.")
          .font(.system(size: 12))
          .foregroundStyle(.secondary)
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
  }
}

// MARK: - Widget mediano: resumen

/// Una fila del resumen: solo el ícono y el valor (el ícono ya dice qué es).
struct Dato: View {
  let icono: String
  let titulo: String
  let valor: String
  let color: Color
  var body: some View {
    HStack(spacing: 10) {
      Image(systemName: icono)
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(.secondary)
        .frame(width: 20)
      Text(valor).font(.system(size: 15, weight: .bold)).foregroundStyle(color).lineLimit(1)
        .minimumScaleFactor(0.8)
    }
    .accessibilityElement(children: .combine)
    .accessibilityLabel("\(titulo): \(valor)")
  }
}

struct VistaResumen: View {
  let estado: EstadoNexo?

  var body: some View {
    HStack(spacing: 14) {
      VistaAudifonos(estado: estado)
      if let e = estado {
        Divider()
        VStack(alignment: .leading, spacing: 10) {
          let principal = e.pantallas.first { $0.interna && $0.brillo != nil } ?? e.pantallas.first { $0.brillo != nil }
          Dato(
            icono: "sun.max.fill", titulo: e.pantallas.count == 1 ? "Pantalla" : "Pantallas",
            valor: principal?.brillo.map { "\($0)%" } ?? "\(e.pantallas.count) en uso",
            color: .primary)
          Dato(
            icono: "dot.radiowaves.left.and.right", titulo: "Bluetooth",
            valor: e.bluetooth ? "Activo" : "Apagado", color: e.bluetooth ? .nexoVerde : .nexoRojo)
          Dato(
            icono: "laptopcomputer.and.iphone", titulo: "Dispositivos",
            valor: "\(e.conectados) de \(e.dispositivos.count)", color: .nexoTurquesa)
        }
        .frame(maxWidth: .infinity, alignment: .leading)
      }
    }
  }
}

// MARK: - Botones de brillo

/// Cada botón avisa a Nexo con una notificación del sistema ("com.ronny.nexo.brillo.<pantalla>.<bajar|subir>").
/// Nexo cambia el brillo y reescribe el estado; aquí se espera ese cambio para que el widget se redibuje con el valor real.
func pedirBrillo(pantalla: Int, subir: Bool) async {
  let antes = fechaEstado()
  notify_post("com.ronny.nexo.brillo.\(pantalla).\(subir ? "subir" : "bajar")")
  for _ in 0..<15 {
    try? await Task.sleep(nanoseconds: 100_000_000)
    if fechaEstado() != antes { break }
  }
}

func fechaEstado() -> Date? {
  (try? FileManager.default.attributesOfItem(atPath: rutaEstado().path))?[.modificationDate] as? Date
}

struct BajarBrillo0: AppIntent {
  static let title: LocalizedStringResource = "Bajar brillo"
  func perform() async throws -> some IntentResult { await pedirBrillo(pantalla: 0, subir: false); return .result() }
}
struct SubirBrillo0: AppIntent {
  static let title: LocalizedStringResource = "Subir brillo"
  func perform() async throws -> some IntentResult { await pedirBrillo(pantalla: 0, subir: true); return .result() }
}
struct BajarBrillo1: AppIntent {
  static let title: LocalizedStringResource = "Bajar brillo de la segunda pantalla"
  func perform() async throws -> some IntentResult { await pedirBrillo(pantalla: 1, subir: false); return .result() }
}
struct SubirBrillo1: AppIntent {
  static let title: LocalizedStringResource = "Subir brillo de la segunda pantalla"
  func perform() async throws -> some IntentResult { await pedirBrillo(pantalla: 1, subir: true); return .result() }
}

extension Color {
  static let nexoAmarillo = Color(red: 0.98, green: 0.75, blue: 0.14)
}

/// Un botón grande y claro: ícono de sol y el signo, para que se entienda sin leer.
struct BotonBrillo<I: AppIntent>: View {
  let intent: I
  let subir: Bool
  var body: some View {
    Button(intent: intent) {
      Image(systemName: subir ? "sun.max.fill" : "sun.min.fill")
        .font(.system(size: subir ? 16 : 13, weight: .semibold))
        .frame(maxWidth: .infinity, minHeight: 30)
      .background(Color.primary.opacity(0.09), in: RoundedRectangle(cornerRadius: 10, style: .continuous))
    }
    .buttonStyle(.plain)
    .accessibilityLabel(subir ? "Subir brillo" : "Bajar brillo")
  }
}

struct Barra: View {
  let nivel: Int
  var body: some View {
    GeometryReader { g in
      ZStack(alignment: .leading) {
        Capsule().fill(Color.primary.opacity(0.12))
        Capsule().fill(Color.nexoAmarillo).frame(width: max(6, g.size.width * CGFloat(nivel) / 100))
      }
    }
    .frame(height: 6)
  }
}

/// Una pantalla con su brillo y los dos botones.
struct FilaBrillo<B: AppIntent, S: AppIntent>: View {
  let pantalla: EstadoNexo.Pantalla
  let bajar: B
  let subir: S
  var compacta = false
  var body: some View {
    VStack(alignment: .leading, spacing: compacta ? 5 : 6) {
      HStack(alignment: .firstTextBaseline) {
        Text(pantalla.brillo.map { "\($0)%" } ?? "—")
          .font(.system(size: compacta ? 22 : 30, weight: .bold, design: .rounded))
          .monospacedDigit()
        Spacer(minLength: 4)
        Image(systemName: pantalla.interna ? "laptopcomputer" : "display")
          .font(.system(size: 13, weight: .medium))
          .foregroundStyle(.secondary)
          .accessibilityLabel(pantalla.interna ? "MacBook" : pantalla.nombre)
      }
      if let nivel = pantalla.brillo {
        Barra(nivel: nivel)
        HStack(spacing: 8) {
          BotonBrillo(intent: bajar, subir: false)
          BotonBrillo(intent: subir, subir: true)
        }
      } else {
        Text("Este monitor no deja cambiar el brillo.")
          .font(.system(size: 11))
          .foregroundStyle(.secondary)
      }
    }
  }
}

struct VistaBrillo: View {
  let estado: EstadoNexo?
  var body: some View {
    VStack(alignment: .leading, spacing: 0) {
      Image(systemName: "sun.max.fill")
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(Color.nexoAmarillo)
        .accessibilityLabel("Brillo")
      Spacer(minLength: 4)
      if let p = estado?.pantallas.first {
        FilaBrillo(pantalla: p, bajar: BajarBrillo0(), subir: SubirBrillo0())
      } else {
        Text("Abre Nexo").font(.system(size: 17, weight: .bold))
        Text("para cambiar el brillo desde aquí.").font(.system(size: 12)).foregroundStyle(.secondary)
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
  }
}

struct VistaPantallas: View {
  let estado: EstadoNexo?
  var body: some View {
    let pantallas = Array((estado?.pantallas ?? []).prefix(2))
    VStack(alignment: .leading, spacing: 8) {
      Image(systemName: "sun.max.fill")
        .font(.system(size: 15, weight: .semibold))
        .foregroundStyle(Color.nexoAmarillo)
        .accessibilityLabel("Brillo")
      if pantallas.isEmpty {
        Text("Abre Nexo para cambiar el brillo desde aquí.").font(.system(size: 12)).foregroundStyle(.secondary)
      } else if pantallas.count == 1 {
        FilaBrillo(pantalla: pantallas[0], bajar: BajarBrillo0(), subir: SubirBrillo0())
      } else {
        HStack(alignment: .top, spacing: 14) {
          FilaBrillo(pantalla: pantallas[0], bajar: BajarBrillo0(), subir: SubirBrillo0(), compacta: true)
          Divider()
          FilaBrillo(pantalla: pantallas[1], bajar: BajarBrillo1(), subir: SubirBrillo1(), compacta: true)
        }
      }
    }
    .frame(maxWidth: .infinity, maxHeight: .infinity, alignment: .topLeading)
  }
}

// MARK: - Widgets

struct WidgetBrillo: Widget {
  var body: some WidgetConfiguration {
    StaticConfiguration(kind: "nexo.brillo", provider: Proveedor()) { entrada in
      VistaBrillo(estado: entrada.estado).containerBackground(for: .widget) { FondoNexo() }
    }
    .configurationDisplayName("Brillo")
    .description("Sube o baja el brillo con un toque.")
    .supportedFamilies([.systemSmall])
  }
}

struct WidgetPantallas: Widget {
  var body: some WidgetConfiguration {
    StaticConfiguration(kind: "nexo.pantallas", provider: Proveedor()) { entrada in
      VistaPantallas(estado: entrada.estado).containerBackground(for: .widget) { FondoNexo() }
    }
    .configurationDisplayName("Brillo de tus pantallas")
    .description("El brillo de la MacBook y del monitor, con botones para subir y bajar.")
    .supportedFamilies([.systemMedium])
  }
}

struct WidgetAudifonos: Widget {
  var body: some WidgetConfiguration {
    StaticConfiguration(kind: "nexo.audifonos", provider: Proveedor()) { entrada in
      VistaAudifonos(estado: entrada.estado).containerBackground(for: .widget) { FondoNexo() }
    }
    .configurationDisplayName("Audífonos")
    .description("La batería y el estado de tus audífonos.")
    .supportedFamilies([.systemSmall])
  }
}

struct WidgetResumen: Widget {
  var body: some WidgetConfiguration {
    StaticConfiguration(kind: "nexo.resumen", provider: Proveedor()) { entrada in
      VistaResumen(estado: entrada.estado).containerBackground(for: .widget) { FondoNexo() }
    }
    .configurationDisplayName("Resumen de Nexo")
    .description("Audífonos, pantallas y Bluetooth de un vistazo.")
    .supportedFamilies([.systemMedium])
  }
}

@main
struct WidgetsNexo: WidgetBundle {
  var body: some Widget {
    WidgetBrillo()
    WidgetAudifonos()
    WidgetPantallas()
    WidgetResumen()
  }
}
