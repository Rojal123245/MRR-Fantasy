/**
 * Squad presets for the futsal randomizer.
 *
 * DATA ONLY. No React, no matching logic, no imports from page.tsx. A preset is
 * a starting position for the two organizer-owned marks; the page resolves it
 * against the live roster and the organizer's saved deltas.
 *
 * ── BAN 1: a preset entry is resolved by `id`, and only by `id`. ──────────────
 *
 * No normalizeName, no toLowerCase() compare, no similarity scoring, no fuzzy
 * fallback — not even as a last resort. The reason is not hypothetical:
 *
 *   The roster holds `Aashis Bhattarai` AND `Ashim Bhattarai`. Two different
 *   people who both turn up to play. The organizer's first draft of the list
 *   below read "Ashish Bhattarai", which scores 0.938 against Aashis and 0.903
 *   against Ashim. Both clear any threshold anyone would call sane, and the gap
 *   between the right human and the wrong human is 0.035.
 *
 * There is no safe threshold to pick, so no threshold is picked. The roster also
 * holds 5 Basnets, 4 Shresthas, 3 Gurungs, Rajeev Lamichhaney beside Arun
 * Lamichhane, 2 Sapkotas, two players first-named Prem, the near-homophones
 * Sabin Regmi and Subin Gajurel — different people — and `Nicolas`, who has no
 * surname at all. A name match here does not fail loudly; it silently marks the
 * wrong human and hands him to the wrong team.
 *
 * ── BAN 2: nothing here mirrors `player.is_top_player`. ──────────────────────
 *
 * That column governs the two-top-player squad limit in the fantasy game
 * (frontend/src/app/team/page.tsx). It is a different set from this list, and
 * measurably so: in production it flags 6 players, two of whom are not on the
 * organizer's list, and it misses 9 of the 13 who are. The randomizer never
 * reads or writes it.
 *
 * ── The `name` field is a display label, never a key. ─────────────────────────
 *
 * It exists so a human can review a diff of otherwise opaque UUIDs, and so an
 * unresolved id can be reported as a person rather than as a hex string. The
 * page emits a dev-only console.warn when a resolved player's roster name
 * differs from the label here, which is the cheapest available guard against
 * pasting a correct name beside a wrong UUID. Nothing in the UI depends on it.
 */

export type PresetEntry = {
  /** The roster UUID. The only lookup key. See BAN 1 above. */
  id: string;
  /** Display label for diffs and the unresolved panel. Never a lookup key. */
  name: string;
};

export type SquadPreset = {
  id: string;
  label: string;
  blurb: string;
  star: readonly PresetEntry[];
  prime: readonly PresetEntry[];
};

/**
 * The group's own list, as the organizer maintains it.
 *
 * Position comments are the roster's primary/secondary at the time of writing.
 * They are documentation for the next person editing this file, not data — the
 * page reads positions from the live roster. They do record something worth
 * knowing, though: the two lists lean opposite ways. `star` is 7 MID and 6 FWD
 * with no defender at all; `prime` is 4 DEF, 3 FWD and 1 MID. That skew is why
 * the balance weights in balance.ts are explicit and tunable rather than tuned.
 */
export const MRR_REGULARS: SquadPreset = {
  id: "mrr-regulars",
  label: "MRR regulars",
  blurb: "13 top players, 8 prime-position. The group's own list.",
  star: [
    { id: "98e24f38-a818-4429-9995-9fb35527eaa5", name: "Ritesh Gurung" },       // FWD/MID
    { id: "16d67d19-0ae4-4c95-b454-9276494b81b8", name: "Aashish Tangnami" },    // MID/FWD
    { id: "e6caefbe-1e78-4159-bb9b-455d6e49b6b4", name: "Aashis Bhattarai" },    // MID/DEF
    { id: "ae201c41-6eb3-4c72-8940-223500ebec03", name: "Rajeev Lamichhaney" },  // FWD/MID
    { id: "2851a189-f8a9-45b6-a474-c035daf7f6aa", name: "Nirmal Gurung" },       // FWD/MID
    { id: "389f9c7c-e928-45d4-8d42-0c000b31f9ba", name: "Sachin Basnet" },       // FWD/DEF
    { id: "c99d32ca-51fc-4b74-80a7-726291b16acd", name: "Pratik Shrestha" },     // MID/FWD
    { id: "fad6df08-fd91-43e5-adca-02703ef60429", name: "Yukesh Shrestha" },     // MID/DEF
    { id: "f6890e4a-ea72-4c20-9d95-93374f1bd8b9", name: "Parbat Rokka" },        // MID/FWD
    { id: "c5d3086d-b438-430d-b65e-ce734afe3918", name: "Sayujya Dhungel" },     // MID/DEF
    { id: "d73ed2b6-ffe0-4934-bad0-81303e8e3c6e", name: "Dilip Magar" },         // FWD/MID
    { id: "944b8a87-f72c-4e4e-86f9-6258dd20c19f", name: "Nicolas" },             // MID/DEF
    { id: "b248044d-a5e8-4df4-842f-6272910c0788", name: "Siddhartha Shrestha" }, // FWD/MID
  ],
  prime: [
    { id: "ad4c5610-fa55-406f-aca1-2686924c7d41", name: "Khagendra Kandel" },   // MID/DEF
    { id: "f1a35b0c-39ef-4873-92fd-33eac42209ff", name: "Dipak Mahatara" },     // FWD/DEF
    { id: "6e36eb7e-d6d4-4c53-bba5-14559fd987ca", name: "Prem Sapkota" },       // FWD/DEF
    { id: "91830a02-76af-4338-927c-b7cb619678ac", name: "Sudarshan Sapkota" },  // FWD/-
    { id: "4f8bf29f-a849-4371-8f0d-bb2f5da95a25", name: "Prasath KKS Thanam" }, // DEF/MID
    { id: "a52ba459-3475-4ecf-a900-31ddbff05d3d", name: "Anish Rana" },         // DEF/MID
    { id: "00ad0ca2-9129-44d9-ad5c-5dbb60745006", name: "Sabin Regmi" },        // DEF/-
    { id: "dcce95bf-ab80-4e6c-a7b7-8dbc34a612bf", name: "Mustafa Arain" },      // DEF/-
  ],
};

/**
 * Decision: "everyone starts unmarked", kept as a permanent one-tap option.
 *
 * A group that does not share the MRR list should not have to un-tap 21 marks
 * before it can use the tool.
 */
export const BLANK_PRESET: SquadPreset = {
  id: "blank",
  label: "Blank",
  blurb: "Nobody marked. Mark players yourself.",
  star: [],
  prime: [],
};

export const PRESETS = [MRR_REGULARS, BLANK_PRESET] as const;

export const DEFAULT_PRESET_ID = "mrr-regulars";
