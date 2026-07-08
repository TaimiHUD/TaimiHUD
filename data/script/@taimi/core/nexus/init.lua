-- stub for testing; core modules are built-in to scripting engine
return {
	available = false,
	supported = true,
	HostSignal = {
		-- pseudo-events (typically explicit addonapi callbacks)
		Event = 40,
		TextureLoad = 41,
		FontLoad = 42,
		RenderOverlayPre = 43,
		RenderOverlay = 44,
		RenderOverlayPost = 45,
		RenderOptions = 46,
		RenderQuickAccess = 47,
		WndProc = 48,
		InputBindPress = 49,
		QuickAccessShortcut = 50,
	},
}
