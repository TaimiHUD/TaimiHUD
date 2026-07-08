local bit32 = bit32
local bitops
local export
if bit32 ~= nil then
	export = bit32
elseif package.loaded["bit32"] or package.preload["bit32"] then
	export = require("bit32")
else
	bitops = bit
	if bitops == nil and package.loaded["bit"] or package.preload["bit"] then
		bitops = require("bit")
	end
end

if export == nil and bitops ~= nil then
	export = {
		tobit = bitops.tobit,
		tohex = bitops.tohex,
		bnot = bitops.bnot,
		bor = bitops.bor,
		band = bitops.band,
		bxor = bitops.bxor,
		lshift = bitops.lshift,
		rshift = bitops.rshift,
		bswap = bitops.bswap,
		-- bit32 compat
		lrotate = bitops.rol,
		rtotate = bitops.ror,
		-- bit exclusive but why not include I guess...
		arshift = bitops.arshift,
	}
	-- bit32 compat
	local bnot, band, bor, lshift, rshift = export.bnot, export.band, export.bor, export.lshift, export.rshift
	function export.btest(...)
		return band(...) ~= 0
	end
	function export.extract(n, field, width)
		width = width or 1
		local mask = rshift(0xffffffff, 32 - width)
		return band(rshift(n, field - 1), mask)
	end
	function export.replace(n, v, field, width)
		width = width or 1
		field = field - 1
		local mask = rshift(0xffffffff, 32 - width)
		v = band(v, mask)
		mask = lshift(mask, field)
		n = band(n, bnot(mask))
		return bor(lshift(v, field))
	end
	package.loaded["bit32"] = export
end

if export == nil then
	export = {}
	require("@taimi/core/log").Warn("fallback bitops lol")

	function export.lshift(v, s)
		if v == 1 then
			return math.pow(2, s)
		else
			error("TODO")
		end
	end
	function export.bor(l, r)
		return l + r
	end
	function export.bnot(v)
		return -x
	end
end

bit32 = export
local mt = { __index = bit32 }
export = setmetatable({}, mt)
function export.trailing0s(v, start)
	for i = start or 0, 31 do
		if bit32.btest(v, bit32.lshift(1, i)) then
			return i
		end
	end
	return 32
end
function export.first1_field(...)
	return export.trailing0s(...) + 1
end
local trailing0s = export.trailing0s
-- for bitindex, bit, remaining in biter(0x1020) do etc end
local function biter_next(v, s)
	local n = trailing0s(v, s)
	if n == 32 then
		return nil
	else
		local bit = bit32.bnot(bit32.lshift(1, n))
		v = bit32.band(v, bit)
		return n, bit, v
	end
end
function export.biter(v, s)
	return biter_next, v, s
end

return export
