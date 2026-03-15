"use client";

import { useState } from "react";
import { useRouter } from "next/navigation";
import Link from "next/link";
import { motion, AnimatePresence } from "framer-motion";
import { Mail, Lock, ArrowRight, ArrowLeft, AlertCircle, CheckCircle, KeyRound } from "lucide-react";
import { login, resetPassword } from "@/lib/api";
import { saveAuth } from "@/lib/auth";

export default function LoginPage() {
  const router = useRouter();
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(false);

  const [showReset, setShowReset] = useState(false);
  const [resetEmail, setResetEmail] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [confirmPassword, setConfirmPassword] = useState("");
  const [resetError, setResetError] = useState("");
  const [resetSuccess, setResetSuccess] = useState("");
  const [resetLoading, setResetLoading] = useState(false);

  const handleSubmit = async (e: React.FormEvent) => {
    e.preventDefault();
    setError("");
    setLoading(true);

    try {
      const res = await login(email, password);
      saveAuth(res.token, res.user);
      router.push("/dashboard");
    } catch (err) {
      setError(err instanceof Error ? err.message : "Login failed");
    } finally {
      setLoading(false);
    }
  };

  const handleReset = async (e: React.FormEvent) => {
    e.preventDefault();
    setResetError("");
    setResetSuccess("");

    if (newPassword !== confirmPassword) {
      setResetError("Passwords do not match");
      return;
    }
    if (newPassword.length < 6) {
      setResetError("Password must be at least 6 characters");
      return;
    }

    setResetLoading(true);
    try {
      const res = await resetPassword(resetEmail, newPassword);
      setResetSuccess(res.message);
      setResetEmail("");
      setNewPassword("");
      setConfirmPassword("");
    } catch (err) {
      setResetError(err instanceof Error ? err.message : "Password reset failed");
    } finally {
      setResetLoading(false);
    }
  };

  const switchToReset = () => {
    setShowReset(true);
    setResetEmail(email);
    setError("");
    setResetError("");
    setResetSuccess("");
  };

  const switchToLogin = () => {
    setShowReset(false);
    setResetError("");
    setResetSuccess("");
    setNewPassword("");
    setConfirmPassword("");
  };

  return (
    <div className="min-h-screen hero-gradient flex items-center justify-center px-4 py-12 relative overflow-hidden">
      {/* Ambient orb */}
      <div className="absolute w-[500px] h-[500px] rounded-full opacity-[0.04] pointer-events-none" style={{ background: "radial-gradient(circle, #00e676, transparent 70%)", top: "10%", left: "-10%" }} />

      <motion.div
        initial={{ opacity: 0, y: 30 }}
        animate={{ opacity: 1, y: 0 }}
        transition={{ duration: 0.7, ease: [0.25, 0.46, 0.45, 0.94] }}
        className="w-full max-w-md relative z-10"
      >
        {/* Logo */}
        <Link href="/" className="flex items-center justify-center gap-3 mb-10 no-underline">
          <div className="w-10 h-10 rounded-xl flex items-center justify-center font-bold text-base shrink-0" style={{ background: "linear-gradient(135deg, var(--accent-green), var(--accent-green-dim))", color: "#000", fontFamily: "var(--font-display)" }}>
            M
          </div>
          <span className="text-xl font-bold tracking-wide" style={{ fontFamily: "var(--font-display)" }}>
            MRR <span style={{ color: "var(--accent-green)" }}>FANTASY</span>
          </span>
        </Link>

        <div className="glass-card no-hover p-8 sm:p-10">
          <AnimatePresence mode="wait">
            {!showReset ? (
              <motion.div
                key="login"
                initial={{ opacity: 0, x: -20 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: 20 }}
                transition={{ duration: 0.3 }}
              >
                <h1 className="text-2xl font-extrabold text-center mb-2" style={{ fontFamily: "var(--font-display)" }}>
                  WELCOME BACK
                </h1>
                <p className="text-center mb-8 text-sm" style={{ color: "var(--text-muted)" }}>
                  Sign in to manage your squad
                </p>

                {error && (
                  <motion.div
                    initial={{ opacity: 0, y: -10 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="flex items-center gap-2 p-3.5 rounded-xl mb-6 text-sm"
                    style={{ background: "rgba(239, 68, 68, 0.08)", border: "1px solid rgba(239, 68, 68, 0.2)", color: "var(--danger)" }}
                  >
                    <AlertCircle size={16} className="shrink-0" />
                    {error}
                  </motion.div>
                )}

                <form onSubmit={handleSubmit} className="space-y-5">
                  <div>
                    <label className="block text-[11px] uppercase tracking-[0.15em] font-semibold mb-2.5" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>Email</label>
                    <div className="relative">
                      <Mail size={18} strokeWidth={1.5} className="absolute left-4 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-muted)" }} />
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
                      <Lock size={18} strokeWidth={1.5} className="absolute left-4 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-muted)" }} />
                      <input
                        type="password"
                        value={password}
                        onChange={(e) => setPassword(e.target.value)}
                        className="input-field pl-12"
                        placeholder="Enter your password"
                        required
                      />
                    </div>
                  </div>

                  <div className="flex justify-end">
                    <button
                      type="button"
                      onClick={switchToReset}
                      className="text-xs font-medium bg-transparent border-none cursor-pointer p-0"
                      style={{ color: "var(--accent-green)" }}
                    >
                      Forgot password?
                    </button>
                  </div>

                  <button
                    type="submit"
                    disabled={loading}
                    className="btn-primary w-full flex items-center justify-center gap-2 text-base disabled:opacity-50 mt-2"
                  >
                    {loading ? "Signing In..." : "Sign In"}
                    {!loading && <ArrowRight size={18} />}
                  </button>
                </form>

                <p className="text-center mt-8 text-sm" style={{ color: "var(--text-muted)" }}>
                  Don&apos;t have an account?{" "}
                  <Link href="/register" className="font-semibold no-underline" style={{ color: "var(--accent-green)" }}>
                    Register
                  </Link>
                </p>
              </motion.div>
            ) : (
              <motion.div
                key="reset"
                initial={{ opacity: 0, x: 20 }}
                animate={{ opacity: 1, x: 0 }}
                exit={{ opacity: 0, x: -20 }}
                transition={{ duration: 0.3 }}
              >
                <div className="flex items-center gap-3 mb-2">
                  <button
                    onClick={switchToLogin}
                    className="w-8 h-8 rounded-lg flex items-center justify-center bg-transparent border-none cursor-pointer transition-colors hover:bg-white/5"
                    style={{ color: "var(--text-muted)" }}
                  >
                    <ArrowLeft size={18} />
                  </button>
                  <h1 className="text-2xl font-extrabold" style={{ fontFamily: "var(--font-display)" }}>
                    RESET PASSWORD
                  </h1>
                </div>
                <p className="mb-8 text-sm" style={{ color: "var(--text-muted)", paddingLeft: "2.75rem" }}>
                  Enter your email and choose a new password
                </p>

                {resetError && (
                  <motion.div
                    initial={{ opacity: 0, y: -10 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="flex items-center gap-2 p-3.5 rounded-xl mb-6 text-sm"
                    style={{ background: "rgba(239, 68, 68, 0.08)", border: "1px solid rgba(239, 68, 68, 0.2)", color: "var(--danger)" }}
                  >
                    <AlertCircle size={16} className="shrink-0" />
                    {resetError}
                  </motion.div>
                )}

                {resetSuccess && (
                  <motion.div
                    initial={{ opacity: 0, y: -10 }}
                    animate={{ opacity: 1, y: 0 }}
                    className="flex items-center gap-2 p-3.5 rounded-xl mb-6 text-sm"
                    style={{ background: "rgba(0, 230, 118, 0.08)", border: "1px solid rgba(0, 230, 118, 0.2)", color: "var(--accent-green)" }}
                  >
                    <CheckCircle size={16} className="shrink-0" />
                    <span>{resetSuccess} <button onClick={switchToLogin} className="font-semibold bg-transparent border-none cursor-pointer p-0 underline" style={{ color: "var(--accent-green)" }}>Sign in now</button></span>
                  </motion.div>
                )}

                <form onSubmit={handleReset} className="space-y-5">
                  <div>
                    <label className="block text-[11px] uppercase tracking-[0.15em] font-semibold mb-2.5" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>Email</label>
                    <div className="relative">
                      <Mail size={18} strokeWidth={1.5} className="absolute left-4 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-muted)" }} />
                      <input
                        type="email"
                        value={resetEmail}
                        onChange={(e) => setResetEmail(e.target.value)}
                        className="input-field pl-12"
                        placeholder="your@email.com"
                        required
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-[11px] uppercase tracking-[0.15em] font-semibold mb-2.5" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>New Password</label>
                    <div className="relative">
                      <KeyRound size={18} strokeWidth={1.5} className="absolute left-4 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-muted)" }} />
                      <input
                        type="password"
                        value={newPassword}
                        onChange={(e) => setNewPassword(e.target.value)}
                        className="input-field pl-12"
                        placeholder="At least 6 characters"
                        required
                        minLength={6}
                      />
                    </div>
                  </div>

                  <div>
                    <label className="block text-[11px] uppercase tracking-[0.15em] font-semibold mb-2.5" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>Confirm Password</label>
                    <div className="relative">
                      <Lock size={18} strokeWidth={1.5} className="absolute left-4 top-1/2 -translate-y-1/2 pointer-events-none" style={{ color: "var(--text-muted)" }} />
                      <input
                        type="password"
                        value={confirmPassword}
                        onChange={(e) => setConfirmPassword(e.target.value)}
                        className="input-field pl-12"
                        placeholder="Re-enter new password"
                        required
                        minLength={6}
                      />
                    </div>
                  </div>

                  <button
                    type="submit"
                    disabled={resetLoading}
                    className="btn-primary w-full flex items-center justify-center gap-2 text-base disabled:opacity-50 mt-2"
                  >
                    {resetLoading ? "Resetting..." : "Reset Password"}
                    {!resetLoading && <ArrowRight size={18} />}
                  </button>
                </form>

                <p className="text-center mt-8 text-sm" style={{ color: "var(--text-muted)" }}>
                  Remember your password?{" "}
                  <button onClick={switchToLogin} className="font-semibold bg-transparent border-none cursor-pointer p-0" style={{ color: "var(--accent-green)" }}>
                    Sign in
                  </button>
                </p>
              </motion.div>
            )}
          </AnimatePresence>
        </div>
      </motion.div>
    </div>
  );
}
