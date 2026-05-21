local util = require"@taimi/util"

local export = {}

local Plug = {
	i = {},
	mt = {},
	loaders = {}
}
function Plug.mt:__index(k)
	local v = Plug.i[k]
	if v == nil then
		v = Plug.loaders[k]
		if v ~= nil then
			v = v(self)
			if v ~= nil then
				rawset(self, k, v)
			end
		end
	end
	return v
end
function Plug.for_plug(plug)
	local i = {
		plug = plug,
	}
	return util.setmetatable(i, Plug.mt)
end
function Plug.loaders:Log()
	return require"@taimi/v0/plug/log".Log.for_plug(self.plug)
end
function Plug.loaders:Loader()
	return require"@taimi/v0/plug/loader".Loader.for_plug(self.plug)
end
function Plug.loaders:Persist()
	return require"@taimi/v0/plug/persist".Persist.for_plug(self.plug)
end
function Plug.i:IsPack()
	-- TODO
	return false
end
function Plug.i:PathingCompat(genv)
	local Context = require"@taimi/compat".Context
	if self:IsPack() then
		return Context.env_for_pack(self.plug, genv or {})
	else
		return Context.env_for_plug(self.plug, genv or {})
	end
end

export.Plug = {
	for_plug = Plug.for_plug,
}

return export
