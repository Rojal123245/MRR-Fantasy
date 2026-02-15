"use client";

import { motion, useMotionValue, useTransform, animate } from "framer-motion";
import { useEffect } from "react";

interface PointsBadgeProps {
  points: number;
  label?: string;
  size?: "sm" | "md" | "lg";
  variant?: "green" | "amber";
}

export default function PointsBadge({ points, label, size = "md", variant = "green" }: PointsBadgeProps) {
  const count = useMotionValue(0);
  const rounded = useTransform(count, (latest) => Math.round(latest));

  useEffect(() => {
    const controls = animate(count, points, {
      duration: 1.2,
      ease: "easeOut",
    });
    return controls.stop;
  }, [count, points]);

  const sizeClasses = {
    sm: "text-xl",
    md: "text-3xl",
    lg: "text-5xl",
  };

  const glowClass = variant === "green" ? "glow-green" : "glow-amber";
  const textColor = variant === "green" ? "var(--accent-green)" : "var(--accent-amber)";

  return (
    <div className={`flex flex-col items-center gap-1 ${glowClass} rounded-2xl p-4`} style={{ background: "var(--bg-card)" }}>
      <motion.span
        className={`${sizeClasses[size]} font-bold`}
        style={{ fontFamily: "var(--font-display)", color: textColor }}
      >
        {rounded}
      </motion.span>
      {label && (
        <span className="text-xs uppercase tracking-wider" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>
          {label}
        </span>
      )}
    </div>
  );
}
