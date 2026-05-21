local nexus = require"@taimi/core/nexus"
local export = {}

local Nexus = {
	i = {}
	mt = {}
	loaders = {}
	Available = nexus.available,
}
function Nexus.mt:__index(k)
	local v = Nexus.i[k]
	if v == nil then
		v = Nexus.loaders[k]
		if v ~= nil then
			v = v(self)
			if v ~= nil then
				rawset(self, k, v)
			end
		end
	end
	return v
end

if Nexus.Available then
	function Nexus.loaders:Signal()
		return require"@taimi/v0/nexus/event".Signal.for_ctx(self.ctx)
	end
	function Nexus.loaders:Event()
		return require"@taimi/v0/nexus/event".Event.for_ctx(self.ctx)
	end
	function Nexus.loaders:Paths()
		return require"@taimi/v0/nexus/paths".Paths.for_plug(self.plug)
	end
	function Nexus.loaders:DataLink()
		return require"@taimi/v0/nexus/datalink".DataLink.for_plug(self.plug)
	end
	function Nexus.loaders:RtApi()
		return require"@taimi/v0/nexus/datalink/rtapi".RtApi.for_plug(self.plug)
	end
	function Nexus.loaders:QuickAccess()
		return require"@taimi/v0/nexus/quickaccess".QuickAccess.for_plug(self.plug)
	end
	function Nexus.loaders:Texture()
		return require"@taimi/v0/nexus/texture".Texture.for_plug(self.plug)
	end
	function Nexus.loaders:InputBind()
		return require"@taimi/v0/nexus/input".Bind.for_plug(self.plug)
	end
end

function Nexus.for_ctx(ctx)
	local i = {
		ctx = ctx,
		plug = ctx.plug,
		Available = Nexus.Available,
	}
	return setmetatable(i, Nexus.mt)
end
export.Nexus = {
	for_ctx = Nexus.for_ctx,
	Available = Nexus.Available,
}

return export
