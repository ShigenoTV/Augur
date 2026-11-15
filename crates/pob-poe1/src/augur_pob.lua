-- Enveloppe d'Augur autour de Path of Building Community (PoE 1), sans fenêtre.
-- Reprend HeadlessWrapper.lua de PoB. Le dossier du paquet est AUGUR_BASE : les
-- chemins relatifs de PoB (Data/…, TreeData/…) y sont recalés, sans toucher au
-- dossier courant du processus.

local base = AUGUR_BASE
local function abs(chemin)
	if type(chemin) ~= "string" or chemin:match("^/") or chemin:match("^%a:[/\\]") then
		return chemin
	end
	return base .. "/src/" .. chemin:gsub("^%./", "")
end
local io_open, io_lines, l_loadfile, l_dofile = io.open, io.lines, loadfile, dofile
io.open = function(chemin, mode) return io_open(abs(chemin), mode) end
io.lines = function(chemin, ...) if chemin == nil then return io_lines() end return io_lines(abs(chemin), ...) end
loadfile = function(chemin) return l_loadfile(abs(chemin)) end
dofile = function(chemin) return l_dofile(abs(chemin)) end
package.path = base .. "/src/?.lua;" .. base .. "/runtime/lua/?.lua;" .. base .. "/runtime/lua/?/init.lua"
package.cpath = ""
-- Aucune requête réseau ni processus externe depuis PoB.
os.execute = function() return nil end
io.popen = nil

arg = {}
dofile("_SimpleGraphic.def.lua")
function GetVirtualScreenSize() return 1920, 1080 end
function GetScriptPath() return base .. "/src" end
function GetRuntimePath() return base .. "/runtime" end
function GetUserPath() return base .. "/user" end
function GetWorkDir() return base .. "/src" end
Inflate = AUGUR_INFLATE
Deflate = AUGUR_DEFLATE
ConPrintf = function() end
ConPrintTable = function() end

__callbackTable__ = { }
function runCallback(name, ...)
	if __callbackTable__[name] then
		return __callbackTable__[name](...)
	elseif __mainObject__ and __mainObject__[name] then
		return __mainObject__[name](__mainObject__, ...)
	end
end
local l_require = require
function require(name)
	if name == "lcurl.safe" then return end
	return l_require(name)
end

dofile("Launch.lua")
__mainObject__.continuousIntegrationMode = false
runCallback("OnInit")
runCallback("OnFrame")
if __mainObject__.promptMsg then error(__mainObject__.promptMsg, 0) end

local function erreur_pob()
	local msg = __mainObject__.promptMsg
	__mainObject__.promptMsg = nil
	return msg
end

local build = nil

-- EHP comme pour PoE 2 : moyenne des plus gros coups encaissables des cinq types.
-- Le TotalEHP de PoB dépend de l'ennemi configuré (boss par défaut) : quand ce
-- dernier tue en un coup, il ne bouge plus quoi qu'on change.
local TYPES_COUPS = { "Physical", "Fire", "Cold", "Lightning", "Chaos" }
local function ehp(out)
	local somme, n = 0, 0
	for _, t in ipairs(TYPES_COUPS) do
		local v = out[t .. "MaximumHitTaken"]
		if type(v) == "number" and v == v and v < math.huge then
			somme, n = somme + v, n + 1
		end
	end
	if n == 0 then return out.TotalEHP or 0 end
	return somme / n
end

function augur_version()
	return launch.versionNumber
end

function augur_load(xml)
	-- Un build sur un arbre absent du paquet est recalé sur le dernier arbre, comme
	-- le fait la conversion de PoB.
	xml = xml:gsub('treeVersion="([^"]+)"', function(v)
		local f = io.open("TreeData/" .. v .. "/tree.lua", "r")
		if f then f:close() return nil end
		return 'treeVersion="' .. latestTreeVersion .. '"'
	end)
	__mainObject__.main:SetMode("BUILD", false, "", xml)
	runCallback("OnFrame")
	local msg = erreur_pob()
	if msg then error(msg, 0) end
	build = __mainObject__.main.modes["BUILD"]
	if not build or not build.calcsTab or not build.calcsTab.mainOutput then
		error("Path of Building n'a pas pu calculer ce build.", 0)
	end
	local out = build.calcsTab.mainOutput
	local skill = build.calcsTab.mainEnv and build.calcsTab.mainEnv.player.mainSkill
	return {
		className = build.spec and build.spec.curClassName or nil,
		ascendancy = build.spec and build.spec.curAscendClassName or nil,
		level = build.characterLevel,
		mainSkill = skill and skill.activeEffect and skill.activeEffect.grantedEffect.name or nil,
		treeVersion = build.spec and build.spec.treeVersion or nil,
		fullDps = out.FullDPS,
		combinedDps = out.CombinedDPS,
		life = out.Life,
		ehp = ehp(out),
	}
end

local RESISTANCES = { "Fire", "Cold", "Lightning", "Chaos" }

local function resume(out)
	local r = {
		fullDps = out.FullDPS or 0,
		combinedDps = out.CombinedDPS or 0,
		life = out.Life or 0,
		energyShield = out.EnergyShield or 0,
		mana = out.Mana or 0,
		ehp = ehp(out),
		resistances = {},
	}
	for _, e in ipairs(RESISTANCES) do
		local final = out[e .. "Resist"] or 0
		r.resistances[e:lower()] = { value = final, cap = final + (out["Missing" .. e .. "Resist"] or 0) }
	end
	return r
end

-- Même règle que matchFlags (Modules/Build.lua, locale à ce fichier dans PoB).
local function matchFlags(reqFlags, notFlags, flags)
	if type(reqFlags) == "string" then reqFlags = { reqFlags } end
	for _, flag in ipairs(reqFlags or {}) do
		if not flags[flag] then return false end
	end
	if type(notFlags) == "string" then notFlags = { notFlags } end
	for _, flag in ipairs(notFlags or {}) do
		if flags[flag] then return false end
	end
	return true
end

-- Lignes de comparaison de PoB (celles de son infobulle), sans les couleurs.
local function lignes(avant, apres)
	local res = {}
	local flags = build.calcsTab.mainEnv.player.mainSkill.skillFlags
	for _, s in ipairs(build.displayStats) do
		if s.stat and s.label and matchFlags(s.flag, s.notFlag, flags) and not s.childStat and s.stat ~= "SkillDPS" then
			local a, b = avant[s.stat] or 0, apres[s.stat] or 0
			if type(a) == "number" and type(b) == "number" and math.abs(b - a) > 0.001 and (not s.condFunc or s.condFunc(b, apres) or s.condFunc(a, avant)) then
				table.insert(res, { label = s.label, before = a, after = b, lowerIsBetter = s.lowerIsBetter and true or false })
			end
		end
	end
	return res
end

function augur_compare(texte)
	if not build then error("Aucun build chargé.", 0) end
	local item = new("Item"):Item(texte)
	if not item.base then return { error = "Path of Building ne reconnaît pas cet objet." } end
	item:BuildModList()
	local calcFunc, avant = build.calcsTab:GetMiscCalculator()
	avant = calcFunc({}, true)
	local principal = item:GetPrimarySlot()
	local emplacements = { principal }
	if principal == "Ring 1" then emplacements = { "Ring 1", "Ring 2" } end
	if principal == "Weapon 1" and item.base.type ~= "Two Handed" and not (item.base.tags and item.base.tags.two_hand_weapon) then
		emplacements = { "Weapon 1", "Weapon 2" }
	end
	if principal == "Jewel" or principal == "Flask 1" then
		return { error = "Les jewels et les flasques ne sont pas encore comparés.", slots = {} }
	end
	local res = { before = resume(avant), slots = {} }
	for _, nom in ipairs(emplacements) do
		local slot = build.itemsTab.slots[nom]
		if slot then
			local porte = build.itemsTab.items[slot.selItemId]
			local apres = calcFunc({ repSlotName = nom, repItem = item }, true)
			table.insert(res.slots, {
				slot = nom,
				equipped = porte and porte.name or nil,
				after = resume(apres),
				lines = lignes(avant, apres),
			})
		end
	end
	return res
end

-- Recherche d'upgrades pondérée : le générateur de PoB (TradeQueryGenerator) teste
-- chaque mod possible de l'emplacement et pèse son gain (Full DPS ×1, EHP ×0,5,
-- les poids par défaut de PoB). Rend la requête du site de trade de PoE 1.
local function vide(t)
	return type(t) ~= "table" or next(t) == nil
end

function augur_upgrade_query(args)
	if not build then error("Aucun build chargé.", 0) end
	local slot = build.itemsTab.slots[args.slot]
	if not slot then return { error = "Emplacement inconnu : " .. tostring(args.slot) } end
	local gen = new("TradeQueryGenerator"):TradeQueryGenerator(build.itemsTab.tradeQuery)
	gen.tradeTypeIndex = 4 -- « online »
	local requete, erreur
	gen.requesterContext = {}
	gen.requesterCallback = function(_, q, e) requete, erreur = q, e end
	gen:StartQuery(slot, {
		statWeights = { { stat = "FullDPS", weightMult = 1.0 }, { stat = "TotalEHP", weightMult = 0.5 } },
		includeCorrupted = false,
		includeMirrored = true,
		includeEldritch = "Remove",
		maxPrice = args.maxPrice,
		maxPriceType = args.maxPriceType,
	})
	if not gen.calcContext or not gen.calcContext.co then
		return { error = "Path of Building ne sait pas chercher cet emplacement." }
	end
	local tours = 0
	while gen.calcContext.co and tours < 10000 do
		gen:OnFrame()
		tours = tours + 1
	end
	if erreur then return { error = erreur } end
	if not requete then return { error = "Recherche non générée." } end
	-- Groupes « et » / « pas » vides : retirés (une table Lua vide ne se distingue
	-- pas d'un objet en JSON).
	local groupes = {}
	for _, g in ipairs(requete.stats) do
		if not vide(g.filters) then table.insert(groupes, g) end
	end
	requete.stats = groupes
	-- Libellés des stats pesées, de la plus forte à la plus faible.
	local textes = {}
	for _, cat in pairs(gen.modData or {}) do
		for _, m in pairs(cat) do
			if type(m) == "table" and m.tradeMod then textes[m.tradeMod.id] = m.tradeMod.text end
		end
	end
	local pseudos = {
		["pseudo.pseudo_total_fire_resistance"] = "+#% total to Fire Resistance",
		["pseudo.pseudo_total_cold_resistance"] = "+#% total to Cold Resistance",
		["pseudo.pseudo_total_lightning_resistance"] = "+#% total to Lightning Resistance",
		["pseudo.pseudo_total_chaos_resistance"] = "+#% total to Chaos Resistance",
		["pseudo.pseudo_total_strength"] = "+# total to Strength",
		["pseudo.pseudo_total_dexterity"] = "+# total to Dexterity",
		["pseudo.pseudo_total_intelligence"] = "+# total to Intelligence",
	}
	for id, t in pairs(pseudos) do textes[id] = t end
	local libelles = {}
	local poids = requete.stats[1] and requete.stats[1].type == "weight" and requete.stats[1].filters or {}
	for i, f in ipairs(poids) do
		if i > 6 then break end
		table.insert(libelles, textes[f.id] or f.id)
	end
	return { query = requete, stats = libelles, filters = #poids }
end
