/* Fake content for the Chronacle app kit. ES module export. */
export const CHRONACLE = {
  campaign: { name: "The Hollow Reach", system: "5e · Homebrew", session: "Session 14" },
  // Sources are organized into collections; a campaign subscribes to the ones it needs.
  collections: [
    { name: "D&D 5 Rules", icon: "book-open", subscribed: true, status: "ok", books: [
      { name: "Player's Handbook (SRD 5.2)", status: "ok" },
      { name: "Dungeon Master's Guide", status: "ok" },
      { name: "Monster Manual", status: "ok" }
    ] },
    { name: "Forgotten Realms", icon: "castle", subscribed: true, status: "ok", books: [
      { name: "Sword Coast Adventurer's Guide", status: "ok" },
      { name: "Codex of the Hollow Reach", status: "ok" }
    ] },
    { name: "Homebrew · Table 3", icon: "scroll-text", subscribed: true, status: "idx", books: [
      { name: "House Rules — Table 3", status: "ok" },
      { name: "Ashen Concord Gazette", status: "idx" }
    ] },
    { name: "Dark Sun", icon: "sun", subscribed: false, status: "off", books: [
      { name: "Wanderer's Journal", status: "off" },
      { name: "Terrors of Athas", status: "off" }
    ] }
  ],
  nav: [
    { id: "oracle", label: "Oracle", icon: "sparkles" }
  ],
  // Notebook mirrors the on-disk layout: sessions/ + entities/<type>/*.md
  categories: [
    { id: "sessions", label: "Sessions", icon: "history", group: "Notebook", folder: "sessions", sub: "Your campaign timeline — recaps, rewards, and open threads." },
    { id: "player_characters", label: "Player Characters", icon: "users-round", group: "Entities", folder: "entities/player_characters", sub: "The party — sheets, hooks, and where each one stands." },
    { id: "npcs", label: "NPCs", icon: "drama", group: "Entities", folder: "entities/npcs", sub: "Everyone the party has met, and a few they haven't yet." },
    { id: "locations", label: "Locations", icon: "map-pin", group: "Entities", folder: "entities/locations", sub: "Places your party has been — and the ones they're avoiding." },
    { id: "factions", label: "Factions", icon: "flag", group: "Entities", folder: "entities/factions", sub: "The powers moving behind your campaign." },
    { id: "creatures", label: "Creatures", icon: "paw-print", group: "Entities", folder: "entities/creatures", sub: "Beasts and horrors stalking the Reach." },
    { id: "items", label: "Items", icon: "gem", group: "Entities", folder: "entities/items", sub: "Artifacts, relics, and loot worth noting." },
    { id: "events", label: "Events", icon: "milestone", group: "Entities", folder: "entities/events", sub: "The moments that shaped the campaign." },
    { id: "misc", label: "Misc", icon: "shapes", group: "Entities", folder: "entities/misc", sub: "Everything else worth keeping." }
  ],
  suggestions: [
    { icon: "swords", text: "Can I cast a spell while grappled?" },
    { icon: "shield", text: "How does cover affect spell attacks?" },
    { icon: "users", text: "Who leads the Ashen Concord?" },
    { icon: "map", text: "What happened at Greywater Ford?" }
  ],
  // pre-seeded thread (a prior exchange)
  seed: [
    { role: "user", text: "How much does a long rest restore?" },
    {
      role: "ruling",
      verdict: "A long rest restores all HP and half your total Hit Dice.",
      why: "After 8 hours (at least 6 sleeping), you regain all lost hit points and recover spent Hit Dice equal to half your total, rounded down — minimum one. You can benefit from only one long rest per 24 hours.",
      cites: [{ label: "SRD 5.2 · Resting", src: "SRD 5.2 · \"Long Rest\"", quote: "A long rest is a period of extended downtime, at least 8 hours long… a character regains all lost hit points. The character also regains spent Hit Dice, up to a number equal to half the character's total." }]
    }
  ],
  // canned answers keyed by loose intent for the fake oracle
  answers: {
    grappl: {
      verdict: "Yes — but at disadvantage.",
      why: "Being grappled reduces your speed to 0 but doesn't stop you casting. A spell with a somatic component still works; however the grappled condition imposes disadvantage on any attack roll the spell requires.",
      cites: [
        { label: "SRD 5.2 · Grappling", src: "SRD 5.2 · \"Grappled\"", quote: "A grappled creature's speed becomes 0… The condition ends if the grappler is incapacitated." },
        { label: "House Rules · T3", src: "House Rules — Table 3 · §2", quote: "Casters may attempt a DC 12 Athletics check as a bonus action to wrench free before casting." }
      ]
    },
    cover: {
      verdict: "Half cover gives +2 AC; three-quarters gives +5.",
      why: "Cover protects against spell attacks the same way it does weapon attacks. A target behind half cover gains +2 AC and Dexterity saves; three-quarters cover grants +5. A target with total cover can't be targeted directly at all.",
      cites: [{ label: "SRD 5.2 · Cover", src: "SRD 5.2 · \"Cover\"", quote: "A target with half cover has a +2 bonus to AC and Dexterity saving throws… A target with total cover can't be targeted directly by an attack or a spell." }]
    },
    concord: {
      verdict: "The Ashen Concord answers to the Pale Magister, Veil Orsanne.",
      why: "The Concord keeps no formal court. Its writ travels by raven and rumor, and its rulings are sealed in grey wax. Orsanne has led it since the Sundering of Greywater Ford.",
      cites: [{ label: "Codex · ch.4", src: "Codex of the Hollow Reach · ch. 4", quote: "The Concord keeps no court. Its writ travels by raven and rumor, and what it decides is sealed in grey wax and never spoken twice." }]
    },
    ford: {
      verdict: "Greywater Ford is where the Concord broke the old Compact.",
      why: "Three winters ago the Concord drowned the Compact's envoys at the crossing rather than treat with them. The Reach has not forgiven it. The Ford is now considered cursed ground by the river-folk.",
      cites: [{ label: "Codex · ch.7", src: "Codex of the Hollow Reach · ch. 7", quote: "At Greywater Ford the river ran grey for a season. No boatman will cross it after dark, and the herons have not returned." }]
    },
    _default: {
      verdict: "Here's what your sources say.",
      why: "I searched your indexed rulebooks and campaign codex for the closest passage. Open the citation to read the exact text, or rephrase and I'll look again.",
      cites: [{ label: "SRD 5.2", src: "SRD 5.2 · index", quote: "Refer to the relevant chapter for the specific rule in question." }]
    }
  },
  // Campaign notebook — entries per category, each with a detail body + meta.
  notes: {
    sessions: [
      { title: "The Drowned Bell", lead: "Session 14 · 12th of Frostmoon", blurb: "A bell tolling beneath Greywater Ford led the party to the Compact's lost envoy — still breathing, after three winters.",
        body: ["The party tracked the tolling to a flooded chapel beneath the Ford and found Envoy Sela Marrow chained below the waterline, kept alive by Concord sorcery.","Freeing her cost them the element of surprise; a Concord raven watched the whole thing and fled north.","Open thread: Sela claims the Grey Seal that bound the Compact was forged, not sworn."],
        meta: { Date: "12 Frostmoon", Location: "Greywater Ford", Reward: "1,200 XP · Concord raven-cipher", Status: "Open thread" }, tags: ["Greywater Ford", "The Compact", "Sela Marrow"] },
      { title: "Grey Wax, Grey Water", lead: "Session 13 · 5th of Frostmoon", blurb: "The party bargained with Old Hesh for passage downriver and learned the Concord had sealed Lastwater shut.",
        body: ["Old Hesh would only run them as far as the reed-line; past that, he said, the water 'remembers the Ford.'","They found Lastwater's docks under Concord seal and slipped in through the boathouse instead."],
        meta: { Date: "5 Frostmoon", Location: "Lastwater", Reward: "900 XP", Status: "Resolved" }, tags: ["Lastwater", "Old Hesh"] },
      { title: "The Heron Does Not Return", lead: "Session 12 · 28th of Mistfall", blurb: "First sign that something beneath the Ford was still awake. A heron, long thought gone, drowned at the party's feet.",
        body: ["The campaign's turn toward the Ford began here, with a dead heron and a rune-stone someone had recently re-cut.","The party agreed to follow the river down, against every warning the Reach-folk gave them."],
        meta: { Date: "28 Mistfall", Location: "Upper Reach", Reward: "800 XP", Status: "Resolved" }, tags: ["The Hollow Reach"] }
    ],
    player_characters: [
      { title: "Brannoch Vane", lead: "Level 5 Cleric of the Tide · played by Mara", blurb: "A storm-priest who left the coast to find out why the river turned grey. Calm until he isn't.",
        body: ["Brannoch carries a bell-shard from a chapel the Concord drowned and rings it before every hard choice.","Bonded to the Compact's cause after meeting Sela; treats the Grey Seal as a personal heresy to undo."],
        meta: { Class: "Cleric (Tempest)", Level: "5", Player: "Mara", AC: "18", HP: "41" }, tags: ["The Compact"] },
      { title: "Sister Kell", lead: "Level 5 Rogue · played by Devon", blurb: "Lapsed Concord acolyte turned thief. Knows the order's hand-signs and hates that she does.",
        body: ["Kell was raised in a Concord cloister and ran the night they pressed her first seal.","She reads Concord ciphers on sight — the party's best lead, and its biggest risk if she's recognized."],
        meta: { Class: "Rogue (Arcane Trickster)", Level: "5", Player: "Devon", AC: "16", HP: "33" }, tags: ["Ashen Concord"] },
      { title: "Yorin Ashfall", lead: "Level 6 Wizard · played by Priya", blurb: "A circuit-mage who treats spells like schematics. Obsessed with how the Grey Seal actually works.",
        body: ["Yorin is convinced the Seal is a device, not an oath, and keeps a notebook of its 'failure modes.'","Half the party's rules questions to Chronacle are really Yorin's."],
        meta: { Class: "Wizard (Artifice)", Level: "6", Player: "Priya", AC: "13", HP: "32" }, tags: ["The Grey Seal"] },
      { title: "Dapple", lead: "Level 5 Druid · played by Sam", blurb: "A fen-druid who speaks for the herons. The only one who could hear the bell beneath the Ford.",
        body: ["Dapple joined to find out why the herons left, and has not forgiven the Ford for it.","Their wild shapes lean aquatic — otter for scouting the flooded chapel, heron for the open water."],
        meta: { Class: "Druid (Wildfen)", Level: "5", Player: "Sam", AC: "15", HP: "38" }, tags: ["The Hollow Reach"] }
    ],
    npcs: [
      { title: "Veil Orsanne", lead: "The Pale Magister · Ashen Concord", blurb: "Rarely seen; never twice in the same face. The will behind the Concord's grey seal.",
        body: ["Orsanne is the Pale Magister — the will behind the Concord's grey seal.","Witnesses cannot agree on Orsanne's face, voice, or even number. Some swear the Magister is three people sharing one title."],
        meta: { Role: "Antagonist", Affiliation: "Ashen Concord", Status: "Active", Threat: "High" }, tags: ["Ashen Concord"] },
      { title: "Sela Marrow", lead: "Compact Envoy · recovered Session 14", blurb: "The envoy the Concord drowned but did not kill. Holds the secret of the Grey Seal's forging.",
        body: ["Kept alive beneath the Ford for three winters by Concord sorcery, for reasons she won't yet say.","Claims the Seal that bound the Compact was forged, not sworn — which would make the whole Sundering a lie."],
        meta: { Role: "Ally (wary)", Affiliation: "The Compact", Status: "Recovering", Threat: "—" }, tags: ["The Compact", "The Grey Seal"] },
      { title: "Old Hesh", lead: "Boatwright of Lastwater", blurb: "Runs the only boat that'll cross the reed-line. Talks to the water like it owes him money.",
        body: ["Hesh ferried the party downriver and won't go past the reed-line; the water, he says, 'remembers the Ford.'","Loyal to the Boatwright's Council and quietly to the Compact."],
        meta: { Role: "Contact", Affiliation: "Boatwright's Council", Status: "Active", Threat: "Low" }, tags: ["Lastwater"] }
    ],
    factions: [
      { title: "The Ashen Concord", lead: "Itinerant order of arbiters", blurb: "Grey-robed arbiters who rule by writ and rumor across the Reach. Keep no court; seal everything in grey wax.",
        body: ["The Ashen Concord keeps no court. Its writ travels by raven and rumor, and what it decides is sealed in grey wax and never spoken twice.","Founded after the Sundering, the Concord positions itself as a neutral arbiter — though the river-folk would call it something colder.","Membership is unknown even to its agents; a Concord seal carries more weight than the magister who pressed it."],
        meta: { Leader: "Veil Orsanne", Seat: "None (itinerant)", Founded: "After the Sundering", Disposition: "Hostile" }, tags: ["Veil Orsanne", "Greywater Ford"] },
      { title: "The Compact", lead: "River-folk alliance · diminished", blurb: "The alliance the Concord betrayed at Greywater Ford. Bitter, patient, and quietly arming.",
        body: ["Once the Compact spoke for every village on the water. After the Ford it speaks for far fewer.","What remains is bitter, patient, and quietly arming — and now has an envoy back."],
        meta: { Leader: "Boatwright's Council", Seat: "Lastwater", Status: "Diminished", Disposition: "Friendly" }, tags: ["Greywater Ford", "Sela Marrow"] }
    ],
    locations: [
      { title: "Greywater Ford", lead: "River crossing · cursed ground", blurb: "Where the Concord broke the old Compact. The river ran grey for a season; the herons never came back.",
        body: ["Three winters ago the Concord drowned the Compact's envoys here rather than treat with them.","The river ran grey for a season. No boatman will cross after dark, and a flooded chapel still tolls beneath it."],
        meta: { Region: "Lower Reach", Status: "Cursed ground", Event: "The Sundering", Danger: "High" }, tags: ["Ashen Concord", "The Compact"] },
      { title: "The Hollow Reach", lead: "Region · the campaign's home", blurb: "A drowned valley of fens, old roads, and sunken keeps. Roads run half a foot below the water.",
        body: ["The Reach is a valley the river never finished taking. Roads run half a foot below the water; keeps lean into the mere.","Its people are stubborn and superstitious, and they keep the old roads marked with rune-stones the Concord pretends not to see."],
        meta: { Type: "Region", Terrain: "Drowned fen", Population: "Sparse", Allegiance: "Independent" }, tags: ["Greywater Ford"] },
      { title: "Lastwater", lead: "Town · Compact seat", blurb: "The last free dock on the river and the Compact's seat. Recently sealed shut by Concord writ.",
        body: ["Lastwater's boathouses are the Compact's heart and its hiding place.","The party found the docks under Concord seal and had to slip in through the boathouse."],
        meta: { Type: "Town", Terrain: "River delta", Population: "Modest", Allegiance: "The Compact" }, tags: ["The Compact", "Old Hesh"] }
    ],
    creatures: [
      { title: "Mere-Drake", lead: "Beast · CR 4 · ambusher", blurb: "A long, eel-bodied fen-dragon that drags prey beneath the reed-line. The Reach's apex predator.",
        body: ["Mere-drakes haunt the flooded roads, surfacing only to feed. They've grown bold since the herons left and nothing thins their young.","A drake took one of Old Hesh's cousins last spring; he still won't run the lower channels alone."],
        meta: { Type: "Beast", CR: "4", AC: "15", HP: "76", Habitat: "Flooded roads" }, tags: ["The Hollow Reach"] },
      { title: "Heron-Wight", lead: "Undead · CR 3 · cursed", blurb: "The drowned herons of Greywater Ford, risen grey and silent. They gather where the bell tolls.",
        body: ["When the river ran grey, the herons that died in it did not stay dead. They wade the shallows of the Ford, stabbing at anything warm.","Dapple cannot speak to them. That, more than anything, is what convinced the party the Ford was truly cursed."],
        meta: { Type: "Undead", CR: "3", AC: "13", HP: "45", Habitat: "Greywater Ford" }, tags: ["Greywater Ford"] },
      { title: "Reed-Stalker", lead: "Monstrosity · CR 1 · swarm-hunter", blurb: "Spindly, mud-colored things that mimic reeds and strike in numbers. Rarely seen alone.",
        body: ["A reed-stalker is almost invisible until it moves. Where there is one, there are a dozen.","The Concord is rumored to drive them ahead of patrols, but no one has proven it."],
        meta: { Type: "Monstrosity", CR: "1", AC: "14", HP: "22", Habitat: "Fens" }, tags: [] }
    ],
    items: [
      { title: "The Grey Seal", lead: "Wondrous item · artifact · unique", blurb: "A disc of grey wax pressed with a sigil no two witnesses describe alike. Makes any decree binding across the Reach.",
        body: ["A disc of grey wax pressed with a sigil no two witnesses describe alike.","To break a Grey Seal unbidden is, by the Concord's own writ, a capital matter. Yorin is convinced it is a device, not an oath — and that devices fail."],
        meta: { Type: "Artifact", Rarity: "Unique", Owner: "Ashen Concord", Attunement: "No" }, tags: ["Ashen Concord", "The Sundering"] },
      { title: "Bell-Shard of the Drowned Chapel", lead: "Wondrous item · rare · attuned", blurb: "A fragment of the bell that tolls beneath Greywater Ford. Rings of its own accord before hard choices.",
        body: ["Brannoch carries the shard and rings it before every grave decision; it answers the Ford's bell, faintly, no matter the distance.","While attuned, it warns of Concord sorcery within 60 feet."],
        meta: { Type: "Wondrous", Rarity: "Rare", Owner: "Brannoch Vane", Attunement: "Yes" }, tags: ["Greywater Ford", "Brannoch Vane"] },
      { title: "Raven-Cipher", lead: "Document · uncommon", blurb: "A recovered Concord cipher-wheel that decodes the order's raven-borne writ. Sister Kell can read it cold.",
        body: ["The Concord's writ travels by raven, sealed in cipher. This wheel turns gibberish into orders.","Recovered Session 14. The party now reads Concord mail one step ahead of the Magister — for as long as the cipher holds."],
        meta: { Type: "Document", Rarity: "Uncommon", Owner: "Sister Kell", Attunement: "No" }, tags: ["Ashen Concord", "Sister Kell"] }
    ],
    events: [
      { title: "The Sundering", lead: "Three winters ago · the founding wound", blurb: "When the Concord broke the old Compact at Greywater Ford and the river ran grey. Everything in the campaign descends from it.",
        body: ["The Concord drowned the Compact's envoys at the Ford rather than treat with them, then sealed the act with grey wax and called it law.","The river ran grey for a season; the herons died; the Compact shattered. The Reach has not forgiven it."],
        meta: { When: "3 winters ago", Where: "Greywater Ford", Type: "Catastrophe", Status: "Past" }, tags: ["Greywater Ford", "The Compact", "Ashen Concord"] },
      { title: "The Drowned Bell Wakes", lead: "Session 14 · present", blurb: "The party freed Sela Marrow and proved the Concord kept her alive beneath the Ford for three winters.",
        body: ["Recovering the envoy turned a cold case into a live war. A Concord raven witnessed it and fled north.","Sela's claim — that the Grey Seal was forged, not sworn — could unmake the Concord's whole authority."],
        meta: { When: "12 Frostmoon", Where: "Greywater Ford", Type: "Turning point", Status: "Active" }, tags: ["Sela Marrow", "The Grey Seal"] }
    ],
    misc: [
      { title: "Concord Hand-Signs", lead: "Reference · cipher", blurb: "The silent gestures Concord agents use to pass writ in public. Sister Kell knows them; the party is learning.",
        body: ["A full grammar of finger-signs and seal-touches, used so a Concord agent never has to speak an order aloud.","Half of Kell's value to the party is reading a room full of grey robes and knowing who just gave the kill-sign."],
        meta: { Type: "Reference", Source: "Sister Kell", Reliability: "High" }, tags: ["Sister Kell", "Ashen Concord"] },
      { title: "Calendar of the Reach", lead: "Reference · setting", blurb: "The river-folk year: Mistfall, Frostmoon, Thawtide, and the long Greywane. Useful for tracking session dates.",
        body: ["The Reach keeps a twelve-month calendar tied to the river's moods rather than the sun.","Current date: 12th of Frostmoon. The Greywane — when the river is lowest and the Ford most exposed — is two months out."],
        meta: { Type: "Reference", Months: "12", Current: "Frostmoon" }, tags: [] }
    ]
  }
};
