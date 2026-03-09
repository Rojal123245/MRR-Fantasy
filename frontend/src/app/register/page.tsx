"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { motion } from "framer-motion";
import { Mail, Lock, User, ArrowRight, AlertCircle } from "lucide-react";
import { register } from "@/lib/api";
import { saveAuth } from "@/lib/auth";

export default function RegisterPage() {
  const router = useRouter();
  const [username, setUsername] = useState("");
  const [fullName, setFullName] = useState("");
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);

    try {
      const res = await register(username, fullName, email, password);
      saveAuth(res.token, res.user);
      router.push("/dashboard");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Registration failed");
    } finally {
      setLoading(false);
    }
  };

  return (
    <div className="min-h-screen hero-gradient flex items-center justify-center px-4 py-12 relative overflow-hidden">
      {/* Ambient orbs */}
      <div className="absolute w-[500px] h-[500px] rounded-full opacity-[0.04] pointer-events-none" style={{ background: "radial-gradient(circle, #f59e0b, transparent 70%)", bottom: "5%", right: "-10%" }} />
      <div className="absolute w-[400px] h-[400px] rounded-full opacity-[0.03] pointer-events-none" style={{ background: "radial-gradient(circle, #00e676, transparent 70%)", top: "5%", left: "-5%" }} />

      <motion.div
        initial={{ opacity: 0, y: 30 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.7, ease: [0.25, 0.46, 0.45, 0.94] }}
        className="w-full max-w-md relative z-10"
      >
        {/* Logo */}
        <Link href="/" className="flex items-center justify-center gap-3 mb-12 no-underline">
          <div className="w-10 h-10 rounded-xl flex items-center justify-center font-bold text-base" style={{ background: "linear-gradient(135deg, var(--accent-green), var(--accent-green-dim))", color: "#000", fontFamily: "var(--font-display)" }}>
            M
          </div>
          <span className="text-xl font-bold tracking-wide" style={{ fontFamily: "var(--font-display)" }}>
            MRR <span style={{ color: "var(--accent-green)" }}>FANTASY</span>
          </span>
        </Link>

        <div className="glass-card p-10">
          <h1 className="text-2xl font-extrabold text-center mb-2" style={{ fontFamily: "var(--font-display)" }}>
            JOIN THE GAME
          </h1>
          <p className="text-center mb-8 text-sm" style={{ color: "var(--text-muted)" }}>
            Create your account and build your dream squad
          </p>

          {error && (
            <motion.div
              initial={{ opacity: 0, y: -10 }}
              animate={{ opacity: 1, y: 0 }}
              className="flex items-center gap-2 p-3.5 rounded-xl mb-6 text-sm"
              style={{ background: "rgba(239, 68, 68, 0.08)", border: "1px solid rgba(239, 68, 68, 0.2)", color: "var(--danger)" }}
            >
              <AlertCircle size={16} />
              {error}
            </motion.div>
          )}

          <form onSubmit={handleSubmit} className="space-y-5">
            <div>
              <label className="block text-[11px] uppercase tracking-[0.15em] font-semibold mb-2.5" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>Username</label>
              <div className="relative">
                <User size={16} className="absolute left-4 top-1/2 -translate-y-1/2" style={{ color: "var(--text-muted)" }} />
                <input
                  type="text"
                  value={username}
                  onChange={(e) => setUsername(e.target.value)}
                  className="input-field pl-12"
                  placeholder="Choose a username"
                  required
                  minLength={3}
                />
              </div>
            </div>

            <div>
              <label className="block text-[11px] uppercase tracking-[0.15em] font-semibold mb-2.5" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>Full Name</label>
              <div className="relative">
                <User size={16} className="absolute left-4 top-1/2 -translate-y-1/2" style={{ color: "var(--text-muted)" }} />
                <input
                  type="text"
                  value={fullName}
                  onChange={(e) => setFullName(e.target.value)}
                  className="input-field pl-12"
                  placeholder="Your real name (e.g. Rojal Pradhan)"
                  required
                />
              </div>
            </div>

            <div>
              <label className="block text-[11px] uppercase tracking-[0.15em] font-semibold mb-2.5" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>Email</label>
              <div className="relative">
                <Mail size={16} className="absolute left-4 top-1/2 -translate-y-1/2" style={{ color: "var(--text-muted)" }} />
                <input
                  type="email"
                  value={email}
                  onChange={(e) => setEmail(e.target.value)}
                  className="input-field pl-12"
                  placeholder="your@email.com"
                  required
                />
              </div>
            </div>

            <div>
              <label className="block text-[11px] uppercase tracking-[0.15em] font-semibold mb-2.5" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>Password</label>
              <div className="relative">
                <Lock size={16} className="absolute left-4 top-1/2 -translate-y-1/2" style={{ color: "var(--text-muted)" }} />
                <input
                  type="password"
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  className="input-field pl-12"
                  placeholder="Min 6 characters"
                  required
                  minLength={6}
                />
              </div>
            </div>

            <button
              type="submit"
              disabled={loading}
              className="btn-primary w-full flex items-center justify-center gap-2 text-base disabled:opacity-50 mt-2"
            >
              {loading ? "Creating Account..." : "Create Account"}
              {!loading && <ArrowRight size={18} />}
            </button>
          </form>

          <p className="text-center mt-8 text-sm" style={{ color: "var(--text-muted)" }}>
            Already have an account?{" "}
            <Link href="/login" className="font-semibold no-underline" style={{ color: "var(--accent-green)" }}>
              Sign In
            </Link>
          </p>
        </div>
      </motion.div>
    </div>
  );
}
