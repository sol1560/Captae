// App 入口：只做浅色（BRIEF 第 1 节），正文默认用 Geist（中文回退到苹方）。
// 系统控件的 tint 用近黑 #171717（计划 4.1）；强调色 #007aff 只在进度条、单选点、链接这些地方单独给。
// 数据：正常打开 Rust 核心（AppModel+Local.swift）。Debug 版带启动参数 `-captaeDemo <编号>-<状态>` 时用演示数据截设计稿，
// 不打开核心；Release 里演示数据和演示路由都不编进去。
import SwiftUI

@main
struct CaptaeApp: App {
  @State private var model: AppModel
  @Environment(\.scenePhase) private var scenePhase
  #if DEBUG
    private let route: DemoRoute?
  #endif

  init() {
    let model = AppModel()
    #if DEBUG
      let route = DemoRoute.fromLaunchArguments()
      route?.apply(to: model)
      self.route = route
      if route == nil { model.startLocal() }
    #else
      model.startLocal()
    #endif
    _model = State(initialValue: model)
  }

  var body: some Scene {
    WindowGroup {
      RootTabs()
        .environment(model)
        .font(.cpBody)
        .tint(Palette.ink)
        .preferredColorScheme(.light)
        .task {
          #if DEBUG
            // 等标签页画好再弹 sheet，和用户点开时一样从下面升上来
            if let route {
              try? await Task.sleep(for: .milliseconds(450))
              route.present(on: model)
            }
          #endif
        }
        .onChange(of: scenePhase) { _, phase in
          switch phase {
          case .active: model.becameActive()
          case .background: model.resignedActive()
          default: break
          }
        }
    }
  }
}
