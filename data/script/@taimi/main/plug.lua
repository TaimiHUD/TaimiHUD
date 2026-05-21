local Main = {}
function Main.plugin_start(plug_info, entrypoint, genv, ...)
	genv.TaimiPlug = plug_info

	local rt = require"@taimi/core/rt"
	if rt.is_unsecured then
		-- normally done by loader regardless?
		setfenv(entrypoint, genv)
	else
		-- shrug
		-- genv_mt.__metatable = {}
	end

	return entrypoint(...)
end

return Main
