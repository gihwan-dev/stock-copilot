import AppKit
import WebKit
import Foundation

final class Runner: NSObject, WKNavigationDelegate {
    let webView: WKWebView
    let window: NSWindow
    init(source: String) {
        let config = WKWebViewConfiguration()
        config.websiteDataStore = .nonPersistent()
        webView = WKWebView(frame: NSRect(x: 0, y: 0, width: 960, height: 640), configuration: config)
        window = NSWindow(contentRect: NSRect(x: 0, y: 0, width: 960, height: 640), styleMask: [.borderless], backing: .buffered, defer: false)
        super.init()
        window.contentView = webView
        webView.navigationDelegate = self
        webView.loadHTMLString("<!doctype html><html><body><script>" + source + "</script></body></html>", baseURL: nil)
    }
    func webView(_ webView: WKWebView, didFinish navigation: WKNavigation!) {
        let script = #"""
        (() => {
          const cases = [
            ['document-scroll', '<div style="width:800px;height:10px"></div>'],
            ['missing-tooltip', '<div data-truncate style="width:60px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">longlonglonglonglonglong</div>'],
            ['valid-tooltip', '<div data-truncate title="longlonglonglong" style="width:40px;overflow:hidden;white-space:nowrap;text-overflow:ellipsis">longlonglonglong</div>'],
            ['fixed-height', '<div style="width:120px;height:20px">긴 글 긴 글 긴 글 긴 글 긴 글 긴 글 긴 글 긴 글</div>'],
            ['flex', '<div style="display:flex;width:120px"><div style="white-space:nowrap">verylongunbreakableenglishwordverylongunbreakableenglishword</div></div>'],
            ['clipped-popup', '<div style="position:absolute;left:50px;top:50px;overflow:hidden;width:100px;height:80px"><div role="dialog" style="position:absolute;left:-20px;top:10px;width:40px;height:30px">팝업</div></div>'],
            ['nested-scroll', '<div data-scroll-viewport style="width:180px;height:120px;overflow:auto"><div data-scroll-viewport style="width:150px;height:80px;overflow:auto"><div style="height:160px"></div></div><div style="height:100px"></div></div>'],
            ['outside-window', '<div role="dialog" style="position:fixed;left:620px;top:40px;width:60px;height:40px">팝업</div>'],
            ['overlap', '<div><button style="width:100px;height:30px">하나</button><button style="width:100px;height:30px;margin-left:-20px">둘</button></div>'],
            ['fixed-locked', '<div role="dialog" style="position:fixed;left:100px;top:100px;width:100px;height:100px"><button id="fixed-inside" style="width:40px;height:30px">확인</button></div>'],
            ['fixed-transformed', '<div style="position:relative;transform:translateZ(0);overflow:hidden;width:60px;height:60px"><button id="fixed-child" style="position:fixed;left:50px;top:10px;width:40px;height:30px">확인</button></div>'],
            ['padding-edge', '<div style="position:absolute;left:100px;top:100px;width:100px;height:80px;border:10px solid;overflow:hidden"><div id="border-child" style="position:absolute;left:-5px;top:10px;width:20px;height:20px"></div></div>']
          ];
          return JSON.stringify(cases.map(([name, body]) => {
            const frame = document.createElement('iframe');
            frame.width = '640'; frame.height = '480'; document.body.append(frame);
            const doc = frame.contentDocument;
            doc.open(); doc.write('<!doctype html><html><head><style>html,body{margin:0;width:100%;height:100%}*{box-sizing:border-box}body{font:16px sans-serif}</style></head><body>' + body + '</body></html>'); doc.close();
            if (name === 'document-scroll') { doc.documentElement.setAttribute('data-scroll-viewport', ''); doc.documentElement.style.overflow = 'auto'; doc.documentElement.append(doc.body.firstElementChild); doc.body.setAttribute('data-scroll-viewport', ''); doc.body.style.overflowX = 'auto'; doc.body.style.overflowY = 'hidden'; }
            if (name === 'fixed-locked') { doc.body.style.height = '20px'; doc.body.style.overflow = 'hidden'; }
            const ids = {'fixed-locked':'fixed-inside', 'fixed-transformed':'fixed-child', 'padding-edge':'border-child'};
            const target = doc.getElementById(ids[name]);
            const rect = target?.getBoundingClientRect();
            const parent = target?.parentElement;
            const parentRect = parent?.getBoundingClientRect();
            const geometry = name === 'fixed-locked' ? rect.top > doc.body.getBoundingClientRect().bottom && rect.right <= doc.documentElement.clientWidth : name === 'fixed-transformed' ? rect.right > parentRect.right : name === 'padding-edge' ? rect.left < parentRect.left + parent.clientLeft : null;
            const findings = dsOverflow.inspectOverflow(doc);
            const mode = doc.compatMode;
            frame.remove(); return {name, mode, geometry, findings};
          }));
        })()
        """#
        webView.evaluateJavaScript(script) { value, error in
            if let error = error { fputs("WKWebView failure: \(error)\n", stderr); exit(1) }
            print(value as? String ?? "no result")
            exit(0)
        }
    }
}
let application = NSApplication.shared
application.setActivationPolicy(.accessory)
let source = try String(contentsOfFile: CommandLine.arguments[1], encoding: .utf8)
let runner = Runner(source: source)
DispatchQueue.main.asyncAfter(deadline: .now() + 20) { fputs("WKWebView timeout\n", stderr); exit(1) }
application.run()
withExtendedLifetime(runner) {}
