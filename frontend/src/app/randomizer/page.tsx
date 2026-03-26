"use client";

import { useEffect, useEffectEvent, useMemo, useRef, useState } from "react";
import { useRouter } from "next/navigation";
import { motion } from "framer-motion";
import {
  AlertCircle,
  LogOut,
  RefreshCw,
  ShieldCheck,
  Shuffle,
  UserPlus,
  Users,
  Wifi,
  WifiOff,
} from "lucide-react";
import Nav from "@/components/nav";
import {
  getRandomizerWebSocketUrl,
  type RandomizerClientMessage,
  type RandomizerParticipant,
  type RandomizerRoomStateMessage,
  type RandomizerServerMessage,
  type User,
} from "@/lib/api";
import { getToken, getUser, isAuthenticated } from "@/lib/auth";

type SocketStatus = "connecting" | "connected" | "disconnected";

export default function RandomizerPage() {
  const router = useRouter();
  const socketRef = useRef<WebSocket | null>(null);

  const [user, setUser] = useState<User | null>(null);
  const [roomState, setRoomState] = useState<RandomizerRoomStateMessage | null>(null);
  const [socketStatus, setSocketStatus] = useState<SocketStatus>("connecting");
  const [teamCount, setTeamCount] = useState(2);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [connectionVersion, setConnectionVersion] = useState(0);

  const sendMessage = useEffectEvent((message: RandomizerClientMessage) => {
    const socket = socketRef.current;
    if (!socket || socket.readyState !== WebSocket.OPEN) {
      setError("Realtime connection is not ready.");
      return;
    }

    socket.send(JSON.stringify(message));
  });

  const handleServerMessage = useEffectEvent((rawMessage: string) => {
    try {
      const message = JSON.parse(rawMessage) as RandomizerServerMessage;
      if (message.type === "room_state") {
        setRoomState(message);
        setLoading(false);
        setError("");
        return;
      }

      if (message.type === "error") {
        setError(message.message);
      }
    } catch {
      setError("Received an invalid randomizer message from the server.");
    }
  });

  useEffect(() => {
    if (!isAuthenticated()) {
      router.push("/login");
      return;
    }

    const token = getToken();
    const currentUser = getUser();
    if (!token || !currentUser) {
      router.push("/login");
      return;
    }

    setUser(currentUser);
    setSocketStatus("connecting");
    setLoading(true);

    let cancelled = false;
    const socket = new WebSocket(getRandomizerWebSocketUrl(token));
    socketRef.current = socket;

    socket.onopen = () => {
      if (cancelled) return;
      setSocketStatus("connected");
    };

    socket.onmessage = (event) => {
      if (cancelled) return;
      if (typeof event.data === "string") {
        handleServerMessage(event.data);
      }
    };

    socket.onerror = () => {
      if (cancelled) return;
      setError("Realtime connection failed.");
      setLoading(false);
    };

    socket.onclose = () => {
      if (cancelled) return;
      if (socketRef.current === socket) {
        socketRef.current = null;
      }
      setSocketStatus("disconnected");
      setLoading(false);
    };

    return () => {
      cancelled = true;
      if (socketRef.current === socket) {
        socketRef.current = null;
      }
      socket.close();
    };
  }, [connectionVersion, router]);

  const currentParticipant = useMemo(
    () => roomState?.participants.find((participant) => participant.user_id === user?.id) ?? null,
    [roomState?.participants, user?.id]
  );

  const categoryCounts = useMemo(() => {
    const participants = roomState?.participants ?? [];
    return participants.reduce(
      (counts, participant) => {
        if (participant.category === "Advance") counts.advance += 1;
        else if (participant.category === "GK") counts.gk += 1;
        else counts.regular += 1;
        return counts;
      },
      { advance: 0, gk: 0, regular: 0 }
    );
  }, [roomState?.participants]);

  const handleReconnect = () => {
    socketRef.current?.close();
    setRoomState(null);
    setError("");
    setConnectionVersion((value) => value + 1);
  };

  const handleJoin = () => {
    setError("");
    sendMessage({ type: "join_room" });
  };

  const handleLeave = () => {
    setError("");
    sendMessage({ type: "leave_room" });
  };

  const handleRandomize = () => {
    setError("");

    if (!Number.isInteger(teamCount) || teamCount <= 0) {
      setError("Please enter a valid team count.");
      return;
    }

    sendMessage({ type: "randomize", team_count: teamCount });
  };

  const participants = roomState?.participants ?? [];
  const teams = roomState?.teams ?? [];

  return (
    <div className="min-h-screen pitch-pattern">
      <Nav />
      <div className="pt-24 pb-12 max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <motion.div initial={{ opacity: 0, y: 16 }} animate={{ opacity: 1, y: 0 }} className="mb-8">
          <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
            <div>
              <h1 className="text-3xl sm:text-4xl font-bold" style={{ fontFamily: "var(--font-display)" }}>
                REALTIME <span style={{ color: "var(--accent-green)" }}>RANDOMIZER</span>
              </h1>
              <p className="mt-2 text-sm max-w-3xl" style={{ color: "var(--text-muted)" }}>
                Join the current match with your account, then randomize only the players who are actually on the
                field. The backend owns the room state and broadcasts the same teams to everyone.
              </p>
            </div>

            <div className="flex flex-wrap items-center gap-3">
              <div
                className="inline-flex items-center gap-2 rounded-full px-3 py-2 text-xs font-semibold"
                style={{
                  background:
                    socketStatus === "connected"
                      ? "rgba(0,230,118,0.12)"
                      : socketStatus === "connecting"
                      ? "rgba(245,158,11,0.12)"
                      : "rgba(255,82,82,0.12)",
                  border:
                    socketStatus === "connected"
                      ? "1px solid rgba(0,230,118,0.25)"
                      : socketStatus === "connecting"
                      ? "1px solid rgba(245,158,11,0.25)"
                      : "1px solid rgba(255,82,82,0.25)",
                }}
              >
                {socketStatus === "connected" ? <Wifi size={14} /> : <WifiOff size={14} />}
                {socketStatus === "connected"
                  ? "Connected"
                  : socketStatus === "connecting"
                  ? "Connecting..."
                  : "Disconnected"}
              </div>

              <button onClick={handleReconnect} className="btn-secondary text-xs inline-flex items-center gap-2">
                <RefreshCw size={14} />
                Reconnect
              </button>
            </div>
          </div>
        </motion.div>

        <div className="grid grid-cols-1 xl:grid-cols-[1.1fr_0.9fr] gap-6">
          <div className="space-y-6">
            <div className="glass-card p-5 sm:p-6">
              <div className="flex flex-col gap-4 lg:flex-row lg:items-center lg:justify-between">
                <div>
                  <h2 className="text-lg font-bold mb-1" style={{ fontFamily: "var(--font-display)" }}>
                    Current Match
                  </h2>
                  <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                    Joined players: {participants.length}
                    {currentParticipant ? ` • You joined as ${currentParticipant.player_name}` : ""}
                  </p>
                </div>

                <div className="flex flex-wrap gap-3">
                  {roomState?.joined ? (
                    <button onClick={handleLeave} className="btn-secondary text-sm inline-flex items-center gap-2">
                      <LogOut size={15} />
                      Leave Match
                    </button>
                  ) : (
                    <button
                      onClick={handleJoin}
                      disabled={socketStatus !== "connected" || !roomState?.can_join}
                      className="btn-primary text-sm inline-flex items-center gap-2 disabled:opacity-50"
                    >
                      <UserPlus size={15} />
                      Join Current Match
                    </button>
                  )}
                </div>
              </div>

              <div className="grid grid-cols-1 md:grid-cols-3 gap-3 mt-5 text-xs">
                <div className="rounded-xl px-3 py-2" style={{ background: "rgba(0,230,118,0.08)" }}>
                  <strong style={{ color: "var(--accent-green)" }}>Advance:</strong> {categoryCounts.advance}
                </div>
                <div className="rounded-xl px-3 py-2" style={{ background: "rgba(245,158,11,0.08)" }}>
                  <strong style={{ color: "var(--accent-amber)" }}>GK:</strong> {categoryCounts.gk}
                </div>
                <div className="rounded-xl px-3 py-2" style={{ background: "rgba(59,130,246,0.08)" }}>
                  <strong style={{ color: "#60a5fa" }}>Regular:</strong> {categoryCounts.regular}
                </div>
              </div>

              <div className="mt-5 max-h-[360px] overflow-y-auto pr-1 space-y-2">
                {participants.length > 0 ? (
                  participants.map((participant) => (
                    <div
                      key={participant.user_id}
                      className="flex items-center justify-between rounded-lg px-3 py-3"
                      style={{ background: "var(--bg-elevated)", border: "1px solid var(--border-color)" }}
                    >
                      <div>
                        <p className="text-sm font-medium">{participant.player_name}</p>
                        <p className="text-xs" style={{ color: "var(--text-muted)" }}>
                          Joined by {participant.username}
                        </p>
                      </div>
                      <span
                        className="text-[10px] font-bold px-2 py-1 rounded-full"
                        style={{
                          fontFamily: "var(--font-display)",
                          background:
                            participant.category === "Advance"
                              ? "rgba(0,230,118,0.18)"
                              : participant.category === "GK"
                              ? "rgba(245,158,11,0.18)"
                              : "rgba(59,130,246,0.18)",
                          color:
                            participant.category === "Advance"
                              ? "var(--accent-green)"
                              : participant.category === "GK"
                              ? "var(--accent-amber)"
                              : "#60a5fa",
                        }}
                      >
                        {participant.category}
                      </span>
                    </div>
                  ))
                ) : (
                  <div
                    className="rounded-xl px-4 py-6 text-sm text-center"
                    style={{ background: "var(--bg-elevated)", border: "1px dashed var(--border-color)" }}
                  >
                    No one has joined the current match yet.
                  </div>
                )}
              </div>
            </div>

            <div className="glass-card p-5 sm:p-6">
              <div className="flex flex-col md:flex-row md:items-end gap-4">
                <div className="w-full md:max-w-xs">
                  <label
                    htmlFor="team-count"
                    className="block text-xs uppercase tracking-[0.14em] mb-2"
                    style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}
                  >
                    Number of Teams
                  </label>
                  <input
                    id="team-count"
                    type="number"
                    min={1}
                    value={teamCount}
                    onChange={(event) => setTeamCount(Number(event.target.value))}
                    className="input-field"
                  />
                  <p className="text-[11px] mt-2" style={{ color: "var(--text-muted)" }}>
                    Only joined players will be randomized.
                  </p>
                </div>

                <button
                  onClick={handleRandomize}
                  disabled={socketStatus !== "connected" || !roomState?.joined || participants.length === 0}
                  className="btn-primary text-sm inline-flex items-center justify-center gap-2 disabled:opacity-50"
                >
                  <Shuffle size={16} />
                  Randomize Teams
                </button>
              </div>
            </div>
          </div>

          <div className="space-y-6">
            {(error || roomState?.join_error) && (
              <div
                className="flex items-start gap-2 p-3 rounded-lg text-sm"
                style={{
                  background: "rgba(255, 82, 82, 0.1)",
                  border: "1px solid rgba(255, 82, 82, 0.3)",
                  color: "var(--danger)",
                }}
              >
                <AlertCircle size={16} className="mt-0.5 shrink-0" />
                <div>
                  {error && <p>{error}</p>}
                  {roomState?.join_error && <p>{roomState.join_error}</p>}
                </div>
              </div>
            )}

            {loading ? (
              <div className="glass-card p-10 flex items-center justify-center">
                <div
                  className="w-8 h-8 border-2 border-t-transparent rounded-full animate-spin"
                  style={{ borderColor: "var(--accent-green)", borderTopColor: "transparent" }}
                />
              </div>
            ) : teams.length > 0 ? (
              <div className="space-y-5">
                {teams.map((team, idx) => {
                  const advance = team.players.filter((player) => player.category === "Advance").length;
                  const gk = team.players.filter((player) => player.category === "GK").length;
                  const regular = team.players.filter((player) => player.category === "Regular").length;

                  return (
                    <motion.div
                      key={`team-${team.team_number}`}
                      initial={{ opacity: 0, y: 12 }}
                      animate={{ opacity: 1, y: 0 }}
                      transition={{ delay: idx * 0.05 }}
                      className="glass-card p-5"
                    >
                      <div className="flex items-center justify-between mb-4">
                        <h2 className="text-xl font-bold" style={{ fontFamily: "var(--font-display)" }}>
                          Team {team.team_number}
                        </h2>
                        <div className="flex items-center gap-3 text-xs" style={{ color: "var(--text-muted)" }}>
                          <span className="inline-flex items-center gap-1">
                            <ShieldCheck size={13} style={{ color: "var(--accent-green)" }} /> A:{advance}
                          </span>
                          <span className="inline-flex items-center gap-1">
                            <Users size={13} style={{ color: "var(--accent-amber)" }} /> GK:{gk}
                          </span>
                          <span>R:{regular}</span>
                        </div>
                      </div>

                      <div className="space-y-2">
                        {team.players.map((player) => (
                          <div
                            key={player.player_id}
                            className="flex items-center justify-between rounded-lg px-3 py-2"
                            style={{ background: "var(--bg-elevated)", border: "1px solid var(--border-color)" }}
                          >
                            <p className="text-sm font-medium">{player.player_name}</p>
                            <span
                              className="text-[10px] font-bold px-2 py-1 rounded-full"
                              style={{
                                fontFamily: "var(--font-display)",
                                background:
                                  player.category === "Advance"
                                    ? "rgba(0,230,118,0.18)"
                                    : player.category === "GK"
                                    ? "rgba(245,158,11,0.18)"
                                    : "rgba(59,130,246,0.18)",
                                color:
                                  player.category === "Advance"
                                    ? "var(--accent-green)"
                                    : player.category === "GK"
                                    ? "var(--accent-amber)"
                                    : "#60a5fa",
                              }}
                            >
                              {player.category}
                            </span>
                          </div>
                        ))}
                      </div>
                    </motion.div>
                  );
                })}
              </div>
            ) : (
              <div className="glass-card p-10 text-center text-sm">
                <p className="text-base font-semibold mb-2" style={{ fontFamily: "var(--font-display)" }}>
                  Waiting For A Live Shuffle
                </p>
                <p style={{ color: "var(--text-muted)" }}>
                  Join the current match, then randomize teams from the joined players when everyone is ready.
                </p>
              </div>
            )}

            {currentParticipant && (
              <div className="glass-card p-5">
                <p className="text-xs uppercase tracking-[0.14em] mb-2" style={{ color: "var(--text-muted)" }}>
                  You Are Playing As
                </p>
                <p className="text-lg font-bold" style={{ fontFamily: "var(--font-display)" }}>
                  {currentParticipant.player_name}
                </p>
                <p className="text-xs mt-1" style={{ color: "var(--text-muted)" }}>
                  Category: {currentParticipant.category}
                </p>
              </div>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}
