"use client";

import { motion } from "framer-motion";
import type { Player } from "@/lib/api";

interface FormationProps {
  players: Player[];
  onRemove?: (player: Player) => void;
}

/**
 * A mini football pitch showing 6 player slots in a formation.
 * Layout: 1 GK - 2 DEF - 2 MID - 1 FWD (or auto-arranged).
 */
export default function Formation({ players, onRemove }: FormationProps) {
  // Create 6 slots and fill with players
  const slots = Array.from({ length: 6 }, (_, i) => players[i] || null);

  // Position coordinates for 6 slots on the pitch (percentage-based)
  const positions = [
    { top: "78%", left: "50%" },  // GK / slot 1
    { top: "58%", left: "25%" },  // DEF left / slot 2
    { top: "58%", left: "75%" },  // DEF right / slot 3
    { top: "38%", left: "25%" },  // MID left / slot 4
    { top: "38%", left: "75%" },  // MID right / slot 5
    { top: "15%", left: "50%" },  // FWD / slot 6
  ];

  return (
    <div className="mini-pitch relative w-full" style={{ paddingBottom: "140%", minHeight: 300 }}>
      {/* Pitch markings overlay */}
      <div className="absolute inset-0 rounded-xl overflow-hidden">
        {/* Center circle */}
        <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-24 h-24 rounded-full border border-white/10" />
        {/* Center line */}
        <div className="absolute top-1/2 left-0 right-0 h-px bg-white/10" />
        {/* Goal area top */}
        <div className="absolute top-0 left-1/2 -translate-x-1/2 w-32 h-12 border-b border-l border-r border-white/10 rounded-b-lg" />
        {/* Goal area bottom */}
        <div className="absolute bottom-0 left-1/2 -translate-x-1/2 w-32 h-12 border-t border-l border-r border-white/10 rounded-t-lg" />
      </div>

      {/* Player slots */}
      {positions.map((pos, i) => (
        <motion.div
          key={i}
          initial={{ opacity: 0, scale: 0.5 }}
          animate={{ opacity: 1, scale: 1 }}
          transition={{ delay: i * 0.1 }}
          className="absolute -translate-x-1/2 -translate-y-1/2 flex flex-col items-center gap-1"
          style={{ top: pos.top, left: pos.left }}
        >
          {slots[i] ? (
            <div
              className="cursor-pointer group"
              onClick={() => onRemove && slots[i] && onRemove(slots[i]!)}
            >
              <div className="w-10 h-10 rounded-full flex items-center justify-center text-xs font-bold border-2 transition-all group-hover:scale-110"
                style={{
                  background: "rgba(0, 230, 118, 0.2)",
                  borderColor: "var(--accent-green)",
                  color: "var(--accent-green)",
                  fontFamily: "var(--font-display)",
                }}
              >
                {slots[i]!.position}
              </div>
              <p className="text-[10px] text-center mt-1 font-medium max-w-[70px] truncate" style={{ color: "white", textShadow: "0 1px 3px rgba(0,0,0,0.8)" }}>
                {slots[i]!.name.split(" ").pop()}
              </p>
            </div>
          ) : (
            <div className="w-10 h-10 rounded-full border-2 border-dashed flex items-center justify-center text-xs" style={{ borderColor: "rgba(255,255,255,0.25)", color: "rgba(255,255,255,0.3)" }}>
              {i + 1}
            </div>
          )}
        </motion.div>
      ))}
    </div>
  );
}
