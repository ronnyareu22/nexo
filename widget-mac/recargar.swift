// Le pide a macOS que redibuje ya los widgets de Nexo. Nexo lo lanza cuando algo cambia y termina enseguida.
import Foundation
import WidgetKit

WidgetCenter.shared.reloadAllTimelines()
// Un momento para que el pedido salga antes de cerrar.
Thread.sleep(forTimeInterval: 0.3)
