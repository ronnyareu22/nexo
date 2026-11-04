// Saca capturas con WebKit, el mismo motor que usa Nexo en la Mac (Chrome a veces dibuja distinto).
// Uso: swift capturar-webkit.swift <ancho> <alto> <carpeta> "nombre|url" ["nombre|url" ...]
import AppKit
import WebKit

let args = CommandLine.arguments
let ancho = Double(args[1])!, alto = Double(args[2])!
let carpeta = args[3]
var pendientes = Array(args.dropFirst(4))

final class Capturador: NSObject, WKNavigationDelegate {
  let vista: WKWebView
  let ventana: NSWindow
  var actual: (String, String)?
  init(ancho: Double, alto: Double) {
    let marco = NSRect(x: 0, y: 0, width: ancho, height: alto)
    vista = WKWebView(frame: marco, configuration: WKWebViewConfiguration())
    ventana = NSWindow(contentRect: marco, styleMask: [.borderless], backing: .buffered, defer: false)
    super.init()
    ventana.contentView = vista
    // En pantalla pero invisible y sin recibir clics: fuera de la pantalla, macOS no la dibuja.
    ventana.alphaValue = 0.01
    ventana.ignoresMouseEvents = true
    ventana.level = .floating
    ventana.setFrameOrigin(NSPoint(x: 0, y: 0))
    ventana.orderFrontRegardless()
    vista.navigationDelegate = self
  }
  func siguiente() {
    guard !pendientes.isEmpty else { NSApp.terminate(nil); return }
    let partes = pendientes.removeFirst().split(separator: "|", maxSplits: 1).map(String.init)
    actual = (partes[0], partes[1])
    vista.load(URLRequest(url: URL(string: partes[1])!))
  }
  func webView(_ w: WKWebView, didFinish n: WKNavigation!) {
    DispatchQueue.main.asyncAfter(deadline: .now() + 2.5) {
      w.takeSnapshot(with: nil) { imagen, _ in
        if let imagen, let tiff = imagen.tiffRepresentation, let rep = NSBitmapImageRep(data: tiff),
          let png = rep.representation(using: .png, properties: [:]) {
          try? png.write(to: URL(fileURLWithPath: "\(carpeta)/\(self.actual!.0).png"))
          print("ok", self.actual!.0)
        }
        self.siguiente()
      }
    }
  }
}

let app = NSApplication.shared
app.setActivationPolicy(.accessory)
let c = Capturador(ancho: ancho, alto: alto)
DispatchQueue.main.async { c.siguiente() }
app.run()
