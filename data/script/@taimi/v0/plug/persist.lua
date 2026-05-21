local util = require"@taimi/util"

local export = {}

local Persist = {
	i = {},
	mt = {},
}
function Persist.mt:__index(k)
	return Persist.i[k]
end
function Persist.new(plug, namespace)
	local i = {
		plug = plug,
		namespace = namespace,
	}
	return util.setmetatable(i, Persist.mt)
end
function Persist.for_plug(plug)
	return Persist.new(plug)
end
function Persist.i:Namespaced(ns)
	if ns == nil then
		error("namespace required", 2)
	elseif self.namespace ~= nil then
		ns = ("%s.%s"):format(self.namespace, ns)
	end
	return Persist.new(self.plug, ns)
end
function Persist.i:SetString(k)
	error("stub: SetString")
end
function Persist.i:GetString(k)
	error("stub: GetString")
end
function Persist.i:UnsetString(k)
	error("stub: UnsetString")
end

export.Persist = {
	for_plug = Persist.for_plug,
}

return export
