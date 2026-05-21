-- TODO: some sort of Taimi all-in-one context for convenience and avoiding require?
local Taimi_i = {}
local export = {}
local mt = {}
local loaders = {}
function mt:__index(k)
	local loader = loaders[k]
	if loader ~= nil then
		loader = loader(self)
		rawset(self, k, loader)
		return loader
	end
	return Taimi_i[k]
end
function loaders.Mumble(Taimi)
	return require"@taimi/v0/mumblelink".Mumble.for_plug(Taimi.ctx.plug)
end
function loaders.Plug(Taimi)
	return require"@taimi/v0/plug".Plug.for_plug(Taimi.ctx.plug)
end
function loaders.Menu(Taimi)
	return require"@taimi/v0/menu".Menu.for_plug(Taimi.ctx.plug)
end
function loaders.Event(Taimi)
	return require"@taimi/v0/event".Event.for_ctx(Taimi.ctx)
end
function loaders.Loader(Taimi)
	return Taimi.Plug.Loader
end
function loaders.Log(Taimi)
	return Taimi.Plug.Log
end
function loaders.Nexus(Taimi)
	return require"@taimi/v0/nexus".Nexus.for_plug(Taimi.ctx.plug)
end
-- TODO: more

function export.new_plug(plug_info)
	local Taimi = {
		TaimiPlug = plug_info,
		ctx = {
			plug = plug_info,
			out = {},
		},
	}
	Taimi.ctx.Taimi = Taimi
	Taimi.ctx.events = require"@taimi/event".EventLoop.new({
		Taimi = Taimi,
		plug = Taimi.ctx.plug,
	})

	return setmetatable(Taimi, mt)
end
return export
