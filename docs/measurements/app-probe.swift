import AppKit
import ApplicationServices
import CoreGraphics
import Darwin
import Foundation

let appPath = ProcessInfo.processInfo.environment["APP_PATH"] ?? "/Applications/Stock Copilot.app"
let executableSuffix = "/Contents/MacOS/src-tauri"

typealias ResponsibleFn = @convention(c) (pid_t) -> pid_t
let responsibleFn: ResponsibleFn? = {
    guard let handle = dlopen(nil, RTLD_NOW),
          let sym = dlsym(handle, "responsibility_get_pid_responsible_for_pid") else { return nil }
    return unsafeBitCast(sym, to: ResponsibleFn.self)
}()

let timebase: (numer: UInt64, denom: UInt64) = {
    var info = mach_timebase_info_data_t()
    mach_timebase_info(&info)
    return (UInt64(info.numer), UInt64(info.denom))
}()

func allPids() -> [pid_t] {
    let count = proc_listallpids(nil, 0)
    var pids = [pid_t](repeating: 0, count: Int(count) * 2)
    let filled = proc_listallpids(&pids, Int32(pids.count * MemoryLayout<pid_t>.size))
    return Array(pids.prefix(Int(filled))).filter { $0 > 0 }
}

func pathOf(_ pid: pid_t) -> String {
    var buf = [CChar](repeating: 0, count: 4096)
    let len = proc_pidpath(pid, &buf, UInt32(buf.count))
    return len > 0 ? String(cString: buf) : ""
}

func appPid() -> pid_t? {
    allPids().first { pathOf($0) == appPath + executableSuffix }
}

func family(of root: pid_t) -> [pid_t] {
    guard let fn = responsibleFn else { return [root] }
    return allPids().filter { $0 == root || fn($0) == root }
}

struct Usage {
    let cpuNanos: UInt64
    let footprint: UInt64
    let idleWakeups: UInt64
    let interruptWakeups: UInt64
}

func usage(_ pid: pid_t) -> Usage? {
    var info = rusage_info_v4()
    let rc = withUnsafeMutablePointer(to: &info) { ptr -> Int32 in
        ptr.withMemoryRebound(to: rusage_info_t?.self, capacity: 1) { proc_pid_rusage(pid, RUSAGE_INFO_V4, $0) }
    }
    guard rc == 0 else { return nil }
    let ticks = info.ri_user_time + info.ri_system_time
    return Usage(
        cpuNanos: ticks * timebase.numer / timebase.denom,
        footprint: info.ri_phys_footprint,
        idleWakeups: info.ri_pkg_idle_wkups,
        interruptWakeups: info.ri_interrupt_wkups
    )
}

func familyUsage(_ root: pid_t) -> (cpu: UInt64, mem: UInt64, idle: UInt64, intr: UInt64, members: Int) {
    var cpu: UInt64 = 0, mem: UInt64 = 0, idle: UInt64 = 0, intr: UInt64 = 0, members = 0
    for pid in family(of: root) {
        if let u = usage(pid) {
            cpu += u.cpuNanos; mem += u.footprint; idle += u.idleWakeups; intr += u.interruptWakeups; members += 1
        }
    }
    return (cpu, mem, idle, intr, members)
}

func onscreenWindowCount(_ pid: pid_t) -> Int {
    guard let list = CGWindowListCopyWindowInfo([.optionOnScreenOnly], kCGNullWindowID) as? [[String: Any]] else { return 0 }
    return list.filter { ($0[kCGWindowOwnerPID as String] as? Int32) == pid && ($0[kCGWindowLayer as String] as? Int) == 0 }.count
}

func nowSeconds() -> Double { Date().timeIntervalSince1970 }

func iso(_ t: Double) -> String {
    let f = ISO8601DateFormatter()
    f.formatOptions = [.withInternetDateTime, .withFractionalSeconds]
    return f.string(from: Date(timeIntervalSince1970: t))
}

func quitApp(timeout: Double = 15) {
    guard let pid = appPid() else { return }
    kill(pid, SIGTERM)
    let end = nowSeconds() + timeout
    while nowSeconds() < end {
        if kill(pid, 0) != 0 { return }
        usleep(50_000)
    }
    kill(pid, SIGKILL)
    usleep(500_000)
}

func launchApp() {
    let task = Process()
    task.executableURL = URL(fileURLWithPath: "/usr/bin/open")
    task.arguments = ["-a", appPath]
    try? task.run()
    task.waitUntilExit()
}

func startup(runs: Int) {
    print("run,launch_to_process_s,launch_to_window_s")
    var results: [Double] = []
    for run in 1...runs {
        quitApp()
        sleep(3)
        let t0 = nowSeconds()
        launchApp()
        var pid: pid_t? = nil
        var tProc = 0.0
        let deadline = t0 + 60
        while nowSeconds() < deadline {
            if pid == nil, let p = appPid() { pid = p; tProc = nowSeconds() }
            if let p = pid, onscreenWindowCount(p) > 0 { break }
            usleep(5_000)
        }
        let tWin = nowSeconds()
        let shown = pid.map { onscreenWindowCount($0) > 0 } ?? false
        let value = shown ? tWin - t0 : -1
        if shown { results.append(value) }
        print(String(format: "%d,%.3f,%.3f", run, pid == nil ? -1 : tProc - t0, value))
        fflush(stdout)
        sleep(5)
    }
    let sorted = results.sorted()
    if !sorted.isEmpty {
        let mean = sorted.reduce(0, +) / Double(sorted.count)
        print(String(format: "summary,n=%d,min=%.3f,median=%.3f,mean=%.3f,max=%.3f", sorted.count, sorted.first!, sorted[sorted.count / 2], mean, sorted.last!))
    }
}

func idle(seconds: Int, interval: Int) {
    guard let pid = appPid() else { print("app-not-running"); exit(2) }
    let start = familyUsage(pid)
    let t0 = nowSeconds()
    var maxMem = start.mem
    print("elapsed_s,members,cpu_pct_interval,footprint_mb")
    var prevCpu = start.cpu, prevT = t0
    while nowSeconds() - t0 < Double(seconds) {
        sleep(UInt32(interval))
        guard kill(pid, 0) == 0 else { print("app-exited"); exit(3) }
        let u = familyUsage(pid)
        let t = nowSeconds()
        maxMem = max(maxMem, u.mem)
        let pct = Double(u.cpu &- prevCpu) / ((t - prevT) * 1e9) * 100
        print(String(format: "%.0f,%d,%.3f,%.1f", t - t0, u.members, pct, Double(u.mem) / 1_048_576))
        fflush(stdout)
        prevCpu = u.cpu; prevT = t
    }
    let end = familyUsage(pid)
    let wall = nowSeconds() - t0
    let avg = Double(end.cpu &- start.cpu) / (wall * 1e9) * 100
    print(String(format: "summary,wall_s=%.0f,avg_cpu_pct=%.3f,max_footprint_mb=%.1f,members=%d", wall, avg, Double(maxMem) / 1_048_576, end.members))
}

func nap(hours: Double, interval: Int) {
    print("time,alive,onscreen_windows,cpu_s_delta,idle_wakeups_delta,interrupt_wakeups_delta,sleep_overrun_s")
    fflush(stdout)
    let t0 = nowSeconds()
    var prev: (cpu: UInt64, mem: UInt64, idle: UInt64, intr: UInt64, members: Int)? = nil
    while nowSeconds() - t0 < hours * 3600 {
        let before = nowSeconds()
        sleep(UInt32(interval))
        let t = nowSeconds()
        let overrun = t - before - Double(interval)
        guard let pid = appPid() else {
            print("\(iso(t)),dead,0,0,0,0,\(String(format: "%.3f", overrun))"); fflush(stdout); prev = nil; continue
        }
        let u = familyUsage(pid)
        if let p = prev {
            print(String(format: "%@,alive,%d,%.3f,%llu,%llu,%.3f", iso(t), onscreenWindowCount(pid), Double(u.cpu &- p.cpu) / 1e9, u.idle &- p.idle, u.intr &- p.intr, overrun))
            fflush(stdout)
        }
        prev = u
    }
}

let args = CommandLine.arguments
switch args.count > 1 ? args[1] : "" {
case "startup": startup(runs: args.count > 2 ? Int(args[2]) ?? 10 : 10)
case "idle": idle(seconds: args.count > 2 ? Int(args[2]) ?? 600 : 600, interval: args.count > 3 ? Int(args[3]) ?? 10 : 10)
case "nap": nap(hours: args.count > 2 ? Double(args[2]) ?? 8 : 8, interval: args.count > 3 ? Int(args[3]) ?? 60 : 60)
case "quit": quitApp()
case "launch": launchApp()
case "close":
    guard let p = appPid() else { print("app-not-running"); exit(2) }
    guard AXIsProcessTrusted() else { print("accessibility-not-trusted"); exit(5) }
    let axApp = AXUIElementCreateApplication(p)
    var windows: CFTypeRef?
    AXUIElementCopyAttributeValue(axApp, kAXWindowsAttribute as CFString, &windows)
    for window in (windows as? [AXUIElement]) ?? [] {
        var button: CFTypeRef?
        AXUIElementCopyAttributeValue(window, kAXCloseButtonAttribute as CFString, &button)
        if let button { print("press", AXUIElementPerformAction(button as! AXUIElement, kAXPressAction as CFString).rawValue) }
    }
    usleep(500_000)
    print("onscreen_windows", onscreenWindowCount(p))
case "hide":
    if let p = appPid(), let running = NSRunningApplication(processIdentifier: p) { print(running.hide() ? "hidden" : "hide-refused"); usleep(500_000); print("onscreen_windows", onscreenWindowCount(p)) } else { print("app-not-running") }
case "family":
    if let p = appPid() { for f in family(of: p) { print(f, pathOf(f)) } } else { print("app-not-running") }
default:
    print("usage: app-probe startup [runs] | idle [seconds] [interval] | nap [hours] [interval] | family | quit | launch | hide | close")
    exit(1)
}
