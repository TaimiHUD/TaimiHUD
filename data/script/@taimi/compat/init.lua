local Context = require"@taimi/compat/env"

return {
	Context = Context,

	pathing_pack_spawn = function(entrypoint, genv)
		entrypoint()
		if genv.Taimi.ctx.out.continuation ~= nil then
			return genv.Taimi.ctx.out.continuation
		else
			local main = genv.Taimi.ctx.events
			main:PrepareMarkerHandlers(genv.Taimi.ctx.plug)
			return main:Runner()
		end
	end
}
