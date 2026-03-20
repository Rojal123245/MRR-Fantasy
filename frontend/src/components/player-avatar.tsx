"use client";

import Image from "next/image";
import { useState } from "react";
import { getPlayerInitials, getPlayerPhotoUrl } from "@/lib/player-photo";

interface PlayerAvatarProps {
  playerName: string;
  sizeClassName?: string;
  className?: string;
}

export default function PlayerAvatar({
  playerName,
  sizeClassName = "w-10 h-10",
  className = "",
}: PlayerAvatarProps) {
  const [failedSrc, setFailedSrc] = useState<string | null>(null);
  const photoSrc = getPlayerPhotoUrl(playerName);
  const imageFailed = failedSrc === photoSrc;

  return (
    <div
      className={`${sizeClassName} rounded-full overflow-hidden flex items-center justify-center font-bold text-[10px] ${className}`}
      style={{ background: "var(--bg-elevated)", color: "var(--text-muted)" }}
    >
      {!imageFailed ? (
        <Image
          src={photoSrc}
          alt={playerName}
          width={80}
          height={80}
          className="w-full h-full object-cover"
          onError={() => setFailedSrc(photoSrc)}
          unoptimized
        />
      ) : (
        <span>{getPlayerInitials(playerName)}</span>
      )}
    </div>
  );
}
