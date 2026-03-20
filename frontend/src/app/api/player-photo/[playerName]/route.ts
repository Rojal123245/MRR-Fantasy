import { promises as fs } from "fs";
import path from "path";
import { NextResponse } from "next/server";
import { normalizePlayerName } from "@/lib/player-photo";

const ALLOWED_EXTENSIONS = new Set([".jpg", ".jpeg", ".png", ".webp"]);
const CONTENT_TYPES: Record<string, string> = {
  ".jpg": "image/jpeg",
  ".jpeg": "image/jpeg",
  ".png": "image/png",
  ".webp": "image/webp",
};

const PHOTOS_DIR = path.join(process.cwd(), "..", "photos");

function getFileExtension(fileName: string): string {
  return path.extname(fileName).toLowerCase();
}

function getBaseName(fileName: string): string {
  return fileName.slice(0, Math.max(0, fileName.length - getFileExtension(fileName).length));
}

export async function GET(
  _request: Request,
  { params }: { params: Promise<{ playerName: string }> }
) {
  const { playerName } = await params;
  const normalizedTarget = normalizePlayerName(decodeURIComponent(playerName));

  if (!normalizedTarget) {
    return NextResponse.json({ error: "Invalid player name" }, { status: 400 });
  }

  try {
    const files = await fs.readdir(PHOTOS_DIR);
    const matchedFile = files.find((file) => {
      const ext = getFileExtension(file);
      if (!ALLOWED_EXTENSIONS.has(ext)) return false;
      return normalizePlayerName(getBaseName(file)) === normalizedTarget;
    });

    if (!matchedFile) {
      return NextResponse.json({ error: "Photo not found" }, { status: 404 });
    }

    const imagePath = path.join(PHOTOS_DIR, matchedFile);
    const imageBuffer = await fs.readFile(imagePath);
    const ext = getFileExtension(matchedFile);

    return new NextResponse(imageBuffer, {
      status: 200,
      headers: {
        "Content-Type": CONTENT_TYPES[ext] ?? "application/octet-stream",
        "Cache-Control": "public, max-age=3600",
      },
    });
  } catch {
    return NextResponse.json({ error: "Failed to load photo" }, { status: 500 });
  }
}
