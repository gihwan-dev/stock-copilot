import AppKit
import WebKit
import Foundation

final class Runner: NSObject, WKNavigationDelegate {
    let webView: WKWebView
    let window: NSWindow
    let checker: String
    var pending: [URL]
    var results: [String] = []
    init(checker: String, urls: [URL]) {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        webView = WKWebView(frame: NSRect(x: 0, y: 0, width: 1440, height: 900), configuration: config)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 1440, height: 900), styleMask: [.borderless], backing: .buffered, defer: false)
        self.checker = checker
        pending = urls
        super.init()
        window.contentView = webView
        webView.navigationDelegate = self
        next()
    }
    func next() {
        guard !pending.isEmpty else {
            print("[" + results.joined(separator: ",") + "]")
            exit(0)
        }
        webView.load(URLRequest(url: pending.removeFirst()))
    }
    func webView(_ webView: WKWebView, didFail navigation: WKNavigation!, withError error: Error) {
        fputs("WKWebView navigation failure: \(error)\n", stderr)
        exit(1)
    }
    func webView(_ webView: WKWebView, didFailProvisionalNavigation navigation: WKNavigation!, withError error: Error) {
        fputs("WKWebView navigation failure: \(error)\n", stderr)
        exit(1)
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        let body = """
        for (let i = 0; i < 300 && globalThis.auditState === undefined; i += 1) {
          await new Promise((done) => setTimeout(done, 50));
        }
        if (globalThis.auditState !== "ready") return JSON.stringify({ state: String(globalThis.auditState) });
        await document.fonts.ready;
        await new Promise((done) => setTimeout(done, 200));
        return JSON.stringify({ state: "ready", url: location.search, result: (\(checker))(document) });
        """
        webView.callAsyncJavaScript(body, arguments: [:], in: nil, in: .page) { outcome in
            switch outcome {
            case .success(let value):
                self.results.append(value as? String ?? "null")
            case .failure(let error):
                fputs("WKWebView failure: \(error)\n", stderr)
                exit(1)
            }
            self.next()
        }
    }
}
let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let checker = try String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
let urls = CommandLine.arguments.dropFirst(2).compactMap { URL(string: $0) }
let runner = Runner(checker: checker, urls: Array(urls))
DispatchQueue.main.asyncAfter(deadline: .now() + 120) { fputs("WKWebView timeout\n", stderr); exit(1) }
application.run()
withExtendedLifetime(runner) {}
