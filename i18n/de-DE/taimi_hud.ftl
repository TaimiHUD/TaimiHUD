## Common

locale-name = Deutsch
join-discord = Discord beitreten
having-issues = Bei Problemen mit TaimiHUD kannst du dich gerne über unseren Discord oder über GitHub-Issues melden!
height = Höhe
font = Schriftart
okay = OK
delete = Löschen
copy = Kopieren
copy-arg = { $arg } kopieren
save = Speichern
quit = Beenden
delete-item = „{ $item }" { delete }?
save-item = „{ $item }" { save }?
save-standalone = Als neue Datei { save }
save-append = An bestehende Datei anhängen
save-edit = Bearbeitungen { save }
save-edit-item = Bearbeitungen in „{ $item }" { save }?
save-mode = Speichermodus
error = Fehler
remove = Entfernen
unknown = Unbekannt
update = Aktualisieren
unset = Nicht gesetzt
add = Hinzufügen
create-arg = Neues { $arg } erstellen
not-create-arg = Vorhandenes { $arg } verwenden
description = Beschreibung
location = Ort: { $path }
data = Daten
object = Objekt
files = Dateien
clear = Zurücksetzen
refresh-files = Dateien aktualisieren
# wie in 3D
model = Modell
revert = Zurücksetzen
close = Schließen
name = Name
icon = Symbol
path = Pfad
title = Titel
menu = Menü
controls = Steuerung
id = ID
category = Kategorie
id-arg = { id }: { $id }
map-id = Karten-{ id }
map-id-arg = { map-id }: { $id }
author = Autor
position = Position
position_cap = Position
not-applicable = k. A.
rt-api-required-base = RTAPI wird benötigt für
rt-api-required = { rt-api-required-base } { $reason }.
no-description = Keine Beschreibung angegeben.
no-thing-arg = Kein(e) { $thing } angegeben.
expand-all = Alle ausklappen
filetype = Dateityp
filename = Dateiname
collapse-all = Alle einklappen
active = Aktiv
inactive = Inaktiv
enable = Aktivieren
cancel = Abbrechen
default = Standard
disable = Deaktivieren
enabled = { enable }t
disabled = { disable }t
author-arg = { author }: { $author }
reset = Zurücksetzen
timer = Timer
timers = { timer }
experimental-notice = Hallo! Diese Funktion ist (größtenteils) experimentell. Einiges kann verwirrend sein und sie erfordert möglicherweise mehr Aufwand als weniger experimentelle Funktionen. Ich entschuldige mich für etwaige Probleme – melde dich gerne auf Discord. – Kat
name-empty = Name leer.
no-trigger = Keine Auslöseposition angegeben.
no-category = Keine Kategorie angegeben.
map-id-wrong = Karten-ID falsch.
no-positions = Keine Markerpositionen angegeben.
validation-fail = Validierung fehlgeschlagen aufgrund von:
filename-empty = Kein Dateiname angegeben.
count = Anzahl
actions = Aktionen
module = Modul

## Addon

primary-window-toggle = Taimi-Fenster ein-/ausblenden
context-menu-primary = { menu }
timer-window-toggle = Timer-Fenster ein-/ausblenden
marker-window-toggle = Marker-Fenster ein-/ausblenden
pathing-window-toggle = Pfad-Fenster ein-/ausblenden
pathing-render-toggle = Pfadanzeige umschalten
pathing-render-minimap-toggle = Minimap-Pfade umschalten
pathing-render-map-toggle = Karten-Pfade umschalten
primary-window-toggle-text = Taimi-Hauptfenster anzeigen/ausblenden
timer-key-trigger = Timer-Tastenauslöser { $id }
timer-key-reset = Timer zurücksetzen

## Config

config-tab = Einstellungen
stock-imgui-progress-bar = Standard-Imgui-Fortschrittsbalken
shadow = Schatten
centre-text-after-icon = Text nach Symbol zentrieren
imgui-notice = Mit Strg+Klick auf ein Schieberegler-Element kann ein Wert direkt eingegeben werden. Nach der Eingabe bitte Enter drücken.
context-click-notice = Rechtsklick für weitere Optionen
dpi-scaling = DPI-Skalierung
dpi-notice = Stelle sicher, dass dies mit der Einstellung {dpi-scaling} in den Grafikoptionen des Spiels übereinstimmt, damit Kartenelemente korrekt angezeigt werden.
marker-trigger = Auslöseverhalten der Markerset-Position
marker-condition = Verhaltensbedingung
autoplace-warning = Wenn RTAPI nicht installiert ist, kann nicht erkannt werden, ob du Leutnant anstatt nur Kommandant bist.
nexus-quick-access = Schnellzugriffs-Symbole
icon-style = Symbolstil
icon-style-plain = Einfach
icon-style-scanlines-1 = Scanlinien
preferred-loader = Loader-Einstellung
preferred-updater = Update-Host-Einstellung
gh-api-token = GitHub-API-Token
gh-api-token-notice = Ratenlimit-Fehler beim Aktualisieren von Datenquellen können durch ein persönliches Token vermieden werden – nur angeben, wenn die Auswirkungen bekannt sind!
language = Sprache
addonbinds = Tastenkürzel
gamebinds = Spielbindungen
keybind = Tastenbindung
gamebind-notice = Diese sollten den Steuerungseinstellungen im Spiel entsprechen. Bei installiertem arcdps-unofficial-extras können sie automatisch erkannt werden.

## Windows

primary-window = TaimiHUD
timers-window = Begegnungs-Timer
markers-window = Squad-Marker
pathing-window = Pfadpakete
# veraltete (?) Aliase
timer-window = { timers-window }
marker-window = { markers-window }

## Modals

addon-uninstall-modal-title = { $source } deinstallieren?
addon-uninstall-modal-button = Deinstallieren
addon-uninstall-modal-description = Bitte Vorsicht! Der Ordner und sein gesamter Inhalt werden gelöscht.
delete-markerset-warning = Bitte Vorsicht! Der Markerset-Eintrag in der Datei wird gelöscht.
overwrite-markerset = Bitte Vorsicht! Der Markerset-Eintrag in der Datei wird überschrieben.

## Openable

open-button = { $kind } öffnen
open-error = { error } beim Öffnen von { $kind }: { $path }

## Data sources

intro-to-data-sources = Bitte das Repository aktualisieren, bevor nach Updates gesucht wird.
data-sources-tab = Datenquellen
data-source-repo-update = Quellen aktualisieren
data-source-repo-update-tooltip = Das Upstream-Datenquellen-Repository abrufen, um herunterladbare Elemente anzuzeigen.
checking-for-updates = Auf Updates prüfen...
downloading-update = Update wird heruntergeladen...
check-for-updates = Auf Updates prüfen
check-for-updates-tooltip = Auf Updates für Datenquellen prüfen. Dies geschieht nicht automatisch, um die Entscheidung über Netzwerkanfragen zu respektieren.
checked-for-updates-last = Zuletzt auf Updates geprüft um: { $time }
reload-data-sources = Datenquellen neu laden
reload-data-sources-tooltip = Elemente aus aktuell installierten Datenquellen neu laden. Nützlich, wenn Dateien manuell geändert wurden!

remote = Remote
update-status = Update-Status
version-installed = Installierte Version: { $version }
version-not-installed = Nicht installiert
update-unknown = Update-Status unbekannt; auf Updates prüfen?
update-not-required = Kein Update erforderlich; aktuell!
update-available = Neue Version verfügbar: { $version }!
update-error = { error } beim Aktualisieren: { $error }!
download = Herunterladen
install = Installieren
attempt-update = Trotzdem versuchen zu aktualisieren?
settings-unloaded = Einstellungen wurden noch nicht geladen!

## Info tab

info-tab = Info
keybind-triggers = Für tastenbasierte Timer-Auslöser bitte die entsprechenden Tasten in den Loader-Einstellungen belegen.
active-timer-phases = Aktive Timer-Phasen
phase = Phase
# Wie in „Spiele-Engine" oder „Render-Engine" :o
engine = Engine
ecs-data = ECS-{ data }
object-data = { object }-{ data }
object-kind = { object }-Typ
model-files = { model }-Dateien
vertices = Vertices
textures = Texturen: { $count }
alloc-size = Zuweisungen: { $size }
d3d-textures = D3D-Texturen: { $count }
size-frag = { $size } { $suffix }
size-frag-mb = { $size } MB
size-frag-kb = { $size } KB

## Arc

arcdps = ArcDPS
arcdps-tab = { arcdps }
nexus = Nexus

## Markers tab

reload-markers = { markers } neu laden
marker-tab = { marker-window }
pathing-tab = { pathing-config }
marker = Marker
markers = { marker }
markers-place = { markers } platzieren
marker-set = { marker }-Set
marker-set-create = { marker-set } erstellen
marker-set-edit = { marker-set } bearbeiten
marker-set-delete = { marker-set } löschen
scaling-factor = Skalierungsfaktor
current-scaling-factor = Aktueller { scaling-factor }: ({ $x }, { $y })
current-scaling-factor-multiple = Aktueller { scaling-factor } als Vielfaches von Fuß pro Kontinenteinheit: ({ $x }, { $y })
scaling-factor-reset = Erkannten { scaling-factor } { reset }
no-file-associated = Zugehörige Datei nicht gefunden
markers-arg = { markers }: { $count }
marker-type = { marker }-Typ
local-header = Lokal (XYZ)
map-header = Karte (XY)
screen-header = Bildschirm (XY)
marker-not-on-screen = Nicht auf dem Bildschirm
select-a-marker = Bitte einen Marker auswählen, um ihn zu konfigurieren!
marker-filetype-explanation = Es gibt drei Arten von Marker-Dateien: die Art, die mit dem BlishHUD-Kommandanten-Marker-Modul kommt (integriert), die Art, die für Community-Marker verwendet wird, und mein eigenes Format, das das Format pro Markerset in eine einzelne Datei pro Markerset umwandelt.
no-markers-for-map = Keine Marker für die aktuelle Karte gefunden.
cant-place-markers = Kann nicht platzieren
autoplacement-disable = Automatische Platzierung deaktivieren
autoplacement-enable = Automatische Platzierung aktivieren

## Markers window

clear-markers = { markers } { clear }
clear-spent-autoplace = Verwendete automatische Platzierungen zurücksetzen

## Edit markers window

edit-markers = Marker erstellen/bearbeiten
set-map-id = Karten-ID auf aktuelle Karte setzen
current-squad-markers = aktuelle Squad-Marker
take-squad-markers = Von { current-squad-markers } übernehmen
cannot-take-squad-markers = Kann nicht von { current-squad-markers } übernehmen; nicht in einem Squad.
rt-api-required-squad-markers = { rt-api-required-base } automatische Übernahme der Squad-Marker-Positionen.
no-position = Keine Position angegeben.
trigger = Auslöser: { $position }
position-plain = { $position }
position-get = Aktuelle { position } ermitteln
set-manually = Manuell setzen
manual-position = Manuelle { position }
set-manually-save = Manuelle { position } { save }
trigger-explanation = Ein Auslöser für ein Markerset ist eine Kugel mit 15 m Radius, deren Mittelpunkt sich an der Auslöserposition befindet.

## Timer tab

reload-timers = { timers } neu laden
timer-tab = { timer-window }
source-arg = Quelle: { $source }
source-adhoc = Quelle: Ad-hoc
select-a-timer = Bitte einen Timer auswählen, um ihn zu konfigurieren!

## Timer window

no-phases-active = Keine Phasen aktiv, keine Timer laufen.
reset-timers = { timers } { reset }

## Pathing

pathing = Pfade
trail = Pfad
poi = POI
space = KatRender
reload-packs = Neu laden
unload-packs = Alle entladen
filter-options = Filteroptionen
searchbar-clear = Suchleiste und Ergebnisse leeren.
show-filter = Filteroptionen anzeigen
hide-filter = Filteroptionen ausblenden
current-map = Aktuelle Karte
ignore-root = Wurzelstatus ignorieren
ignore-leaf = Blattstatus ignorieren
ignore-branch = Zweigstatus ignorieren
show-hidden = Versteckte anzeigen
show-all = Alle anzeigen
ignore-whitespace = Leerzeichen ignorieren
case-insensitive = Groß-/Kleinschreibung ignorieren
toggle = Umschalten
pathing-config = Pfadoptionen
pathing-config-enable = {space}-Pfade
pathing-config-minimap = Minimap-Optionen
pathing-config-worldmap = Kartenoptionen
pathing-config-trail-alpha = Deckkraft
pathing-config-trail-alpha-minimap = Minimap-Deckkraft
pathing-config-trail-alpha-worldmap = Karten-Deckkraft
pathing-config-poi-alpha = Billboard-Deckkraft
pathing-config-poi-alpha-minimap = POI-Minimap-Deckkraft
pathing-config-poi-alpha-worldmap = POI-Deckkraft
pathing-config-trail-scale = Skalierung
pathing-config-trail-scale-minimap = Minimap-Skalierung
pathing-config-trail-scale-worldmap = Kartenskalierung
pathing-config-poi-scale = Billboard-Größe
pathing-config-poi-scale-minimap = POI-Minimap-Größe
pathing-config-poi-scale-worldmap = POI-Größe
pathing-config-player-overlap-threshold = In Spielernähe ausblenden
pathing-config-distance-fade-intensity = Intensität
pathing-config-distance-max = Entfernung
pathing-config-textured = Texturierte Pfade
pathing-config-textured-minimap = Texturierte Pfade
pathing-config-textured-worldmap = Texturierte Pfade
pathing-config-map-open = Fwoom
pathing-config-camera-source = Kameradatenquelle
pathing-config-advanced = Erweiterte Einstellungen
pathing-config-trail-notice = Pfad-Generierungseinstellungen erfordern möglicherweise einen Kartenwechsel oder Neu-Laden und funktionieren eventuell nicht wie erwartet.
pathing-config-trail-y-offset = Vertikaler Versatz
pathing-config-trail-resolution = Pfadauflösung
pathing-config-trail-width = Basisbreite
pathing-config-goggles = Röntgenbrille-Experiment
pathing-config-goggles-notice = Aktuell ist dafür die Einstellung „Render-Sampling: Nativ" in den Grafikoptionen erforderlich.
pathing-config-festivals = {festival}s
pathing-config-festival-active = {$festival} (aktiv)
pathing-config-reset-notice = Rechtsklick auf einen Schieberegler, um ihn auf den Standardwert zurückzusetzen.
pathing-notice-space = {space} wird für die Pfadfunktionalität benötigt.

## Space

render-unload = Render entladen
render-reload = Render neu laden
render-notice-gameplay = Lade ins Spiel, um zu beginnen
render-notice-gameplay-initial = Wähle einen Charakter, um zu beginnen
render-notice-error = Fehler! Weitere Details im Log unter Nexus oder im Taimi-Addon-Ordner
packs-empty = Keine Dateien geladen
packs-empty-notice = Nach der Installation über den Tab „{ data-sources-tab }" oder manuellem Download sollte die Schaltfläche „Neu laden" sie erkennen!

## Festivals

festival = Festival
halloween = Halloween
wintersday = Wintersday
superadventurefestival = Super-Abenteuerkiste
lunarnewyear = Chinesisches Neujahr
festivalofthefourwinds = Festival der Vier Winde
dragonbash = Drachenfest

## Gamebinds (see `default_keybind`s in src/exports/runtime/bindings/controls.rs)
UI_ShowHideUI = UI anzeigen/ausblenden
Map_OpenClose = Karte
Map_Recenter = Neu zentrieren
gamebind-marker-arrow = Pfeil
gamebind-marker-circle = Kreis
gamebind-marker-heart = Herz
gamebind-marker-square = Viereck
gamebind-marker-star = Stern
gamebind-marker-spiral = Spirale
gamebind-marker-triangle = Dreieck
gamebind-marker-x = X
#gamebind-marker-clear = Alle {$kind} entfernen
#gamebind-marker-location-suffix = {" "}(Ortsmarkierungen)
gamebind-marker-location-suffix = {""}
gamebind-marker-object-suffix = {" "}(Objektmarkierungen)
# common
Squad_Location_Arrow = {gamebind-marker-arrow}{gamebind-marker-location-suffix}
Squad_Location_Circle = {gamebind-marker-circle}{gamebind-marker-location-suffix}
Squad_Location_Heart = {gamebind-marker-heart}{gamebind-marker-location-suffix}
Squad_Location_Square = {gamebind-marker-square}{gamebind-marker-location-suffix}
Squad_Location_Star = {gamebind-marker-star}{gamebind-marker-location-suffix}
Squad_Location_Spiral = {gamebind-marker-spiral}{gamebind-marker-location-suffix}
Squad_Location_Triangle = {gamebind-marker-triangle}{gamebind-marker-location-suffix}
Squad_Location_X = {gamebind-marker-x}{gamebind-marker-location-suffix}
Squad_Object_Arrow = {gamebind-marker-arrow}{gamebind-marker-object-suffix}
Squad_Object_Circle = {gamebind-marker-circle}{gamebind-marker-object-suffix}
Squad_Object_Heart = {gamebind-marker-heart}{gamebind-marker-object-suffix}
Squad_Object_Square = {gamebind-marker-square}{gamebind-marker-object-suffix}
Squad_Object_Star = {gamebind-marker-star}{gamebind-marker-object-suffix}
Squad_Object_Spiral = {gamebind-marker-spiral}{gamebind-marker-object-suffix}
Squad_Object_Triangle = {gamebind-marker-triangle}{gamebind-marker-object-suffix}
Squad_Object_X = {gamebind-marker-x}{gamebind-marker-object-suffix}
Squad_ClearAllLocationMarkers = Alle Ortsmarkierungen entfernen
Squad_ClearAllObjectMarkers = Alle Objektmarkierungen entfernen

locale-name = Deutsch
#locale-name-de = {locale-name}
#locale-name-en = Englisch
#locale-name-fr = Französisch
#locale-name-es = Spanisch
