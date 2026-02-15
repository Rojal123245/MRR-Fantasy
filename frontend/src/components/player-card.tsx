"use client";

import { motion } from "framer-motion";
import { User, Zap, Crown } from "lucide-react";
import type { Player } from "@/lib/api";

interface PlayerCardProps {
  player: Player;
  selected?: boolean;
  onSelect?: (player: Player) => void;
  onRemove?: (player: Player) => void;
  delay?: number;
}

const positionColors: Record<string, string> = {
  GK: "badge-gk",
  DEF: "badge-def",
  MID: "badge-mid",
  FWD: "badge-fwd",
};

export default function PlayerCard({ player, selected, onSelect, onRemove, delay = 0 }: PlayerCardProps) {
  return (
    <motion.div
      initial={{ opacity: 0, y: 20 }}
      animate={{ opacity: 1, y: 0 }}
      transition={{ duration: 0.4, delay }}
      whileHover={{ scale: 1.02 }}
      className={`glass-card p-4 cursor-pointer relative overflow-hidden ${selected ? "ring-2 ring-[var(--accent-green)]" : ""} ${player.is_top_player ? "ring-1 ring-[rgba(255,215,0,0.4)]" : ""}`}
      onClick={() => {
        if (selected && onRemove) onRemove(player);
        else if (onSelect) onSelect(player);
      }}
    >
      {/* Subtle gradient overlay */}
      <div className="absolute inset-0 opacity-30" style={{
        background: `radial-gradient(circle at top right, ${
          player.position === "FWD" ? "rgba(239,68,68,0.15)" :
          player.position === "MID" ? "rgba(16,185,129,0.15)" :
          player.position === "DEF" ? "rgba(59,130,246,0.15)" :
          "rgba(245,158,11,0.15)"
        }, transparent 70%)`
      }} />

      <div className="relative flex items-center gap-3">
        {/* Player avatar placeholder */}
        <div className="relative">
          <div className={`w-12 h-12 rounded-full flex items-center justify-center ${player.is_top_player ? "ring-2 ring-amber-400/60" : ""}`} style={{ background: "var(--bg-elevated)" }}>
            <User size={24} style={{ color: player.is_top_player ? "#fbbf24" : "var(--text-muted)" }} />
          </div>
          {player.is_top_player && (
            <div className="absolute -top-1.5 -right-1.5 w-5 h-5 rounded-full flex items-center justify-center" style={{ background: "linear-gradient(135deg, #fbbf24, #f59e0b)", boxShadow: "0 0 8px rgba(251,191,36,0.5)" }}>
              <Crown size={11} style={{ color: "#1a1a2e" }} strokeWidth={2.5} />
            </div>
          )}
        </div>

        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <span className={`${positionColors[player.position]} text-xs font-bold px-2 py-0.5 rounded-full text-white`}>
              {player.position}
            </span>
            {player.secondary_position && (
              <span className={`${positionColors[player.secondary_position]} text-xs font-bold px-2 py-0.5 rounded-full text-white opacity-70`}>
                {player.secondary_position}
              </span>
            )}
            <span className="text-sm font-semibold truncate" style={{ color: "var(--text-primary)", fontFamily: "var(--font-display)" }}>
              {player.name}
            </span>
          </div>
          <p className="text-xs m-0" style={{ color: "var(--text-muted)" }}>
            {player.team_name}
          </p>
        </div>

        <div className="text-right">
          <div className="flex items-center gap-1">
            <Zap size={14} style={{ color: "var(--accent-amber)" }} />
            <span className="text-lg font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-amber)" }}>
              {player.total_points}
            </span>
          </div>
          <p className="text-xs m-0" style={{ color: "var(--text-muted)" }}>
            ${player.price}
          </p>
        </div>
      </div>

      {selected && (
        <motion.div
          initial={{ scale: 0 }}
          animate={{ scale: 1 }}
          className="absolute top-2 right-2 w-6 h-6 rounded-full flex items-center justify-center text-xs font-bold"
          style={{ background: "var(--accent-green)", color: "var(--bg-primary)" }}
        >
          ✓
        </motion.div>
      )}
    </motion.div>
  );
}
