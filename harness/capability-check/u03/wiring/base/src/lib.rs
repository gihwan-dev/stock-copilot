fn build_app(builder: Builder, mut context: Context) {
    for window in &mut context.config_mut().app.windows {
        window.visible = false;
        window.create = false;
    }
    builder.setup(move |app| {
        ipc::registry::specta_builder::<R>().mount_events(app);
        security::create_windows(app)?;
        Ok(())
    }).build(context)
}
