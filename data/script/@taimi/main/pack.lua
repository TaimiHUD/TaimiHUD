local Main = {}
function Main.pathing_pack_start(pack_info, entrypoint, genv)
	genv.TaimiPlug = pack_info
	local compat = require"@taimi/compat"
	compat.Context.env_for_pack(pack_info, genv, genv)

	local rt = require"@taimi/core/rt"
	-- TODO: if this is ever actually needed, do it inside rt?
	if rt.is_unsecured then
		-- normally done by loader regardless?
		setfenv(entrypoint, genv)
	else
		-- shrug
		-- genv_mt.__metatable = {}
	end

	return compat.pathing_pack_spawn(entrypoint, genv)
end

return Main
