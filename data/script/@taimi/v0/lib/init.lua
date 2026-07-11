local export = {}

local Lib = {
	i = {},
	mt = {},
	requires = {
		lson = "@taimi/todo/lson",
	},
	loaders = {},
}
function Lib.mt:__index(k)
	local v = Lib.i[k]
	if v == nil then
		v = Lib.requires[k]
		if v ~= nil then
			v = require(v)
			rawset(self, k, v)
		end
	end
	if v == nil then
		v = Lib.loaders[k]
		if v ~= nil then
			v = v(self)
			rawset(self, k, v)
		end
	end
	return v
end
function Lib.for_ctx(ctx)
	local i = {
		ctx = ctx,
		plug = ctx.plug,
	}
	return setmetatable(i, Lib.mt)
end

export.Lib = {
	for_ctx = Lib.for_ctx,
}

return export
