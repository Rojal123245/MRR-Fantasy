"use client";

import Link from "next/link";
import { motion } from "framer-motion";
import { Trophy, Users, Shield, Zap, ChevronRight, Star, Hexagon } from "lucide-react";

const features = [
  {
    icon: Shield,
    title: "Build Your Squad",
    description: "Draft 6 elite footballers across GK, DEF, MID, FWD. Choose your formation. Pick your captain.",
    accent: "#00e676",
  },
  {
    icon: Zap,
    title: "Earn Points",
    description: "Goals, assists, clean sheets, saves, tackles — every action counts toward your weekly score.",
    accent: "#f59e0b",
  },
  {
    icon: Users,
    title: "Create Leagues",
    description: "Private leagues with invite codes. Go head-to-head with friends and prove your tactics.",
    accent: "#3b82f6",
  },
  {
    icon: Trophy,
    title: "Climb the Ranks",
    description: "Live leaderboards updated every match week. One goal can change everything.",
    accent: "#ef4444",
  },
];

const containerVariants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: { staggerChildren: 0.12 },
  },
};

const itemVariants = {
  hidden: { opacity: 0, y: 40 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.7, ease: [0.25, 0.46, 0.45, 0.94] as const } },
};

export default function LandingPage() {
  return (
    <div className="min-h-screen" style={{ background: "var(--bg-primary)" }}>
      {/* ── HERO SECTION ── */}
      <section className="hero-gradient floodlight relative overflow-hidden min-h-screen flex flex-col">
        {/* Ambient orbs */}
        <div className="absolute inset-0 overflow-hidden pointer-events-none">
          <div className="absolute w-[600px] h-[600px] rounded-full opacity-[0.04]" style={{ background: "radial-gradient(circle, #00e676, transparent 70%)", top: "-10%", left: "-10%" }} />
          <div className="absolute w-[500px] h-[500px] rounded-full opacity-[0.03]" style={{ background: "radial-gradient(circle, #f59e0b, transparent 70%)", bottom: "-5%", right: "-10%" }} />
          <div className="absolute top-32 left-[15%] w-1 h-1 rounded-full bg-[var(--accent-green)] opacity-40 float-animation" />
          <div className="absolute top-48 right-[20%] w-1.5 h-1.5 rounded-full bg-[var(--accent-amber)] opacity-25 float-animation" style={{ animationDelay: "2s" }} />
          <div className="absolute bottom-[30%] left-[40%] w-1 h-1 rounded-full bg-[var(--accent-green)] opacity-30 float-animation" style={{ animationDelay: "4s" }} />
        </div>

        {/* Top bar */}
        <div className="relative z-10 max-w-7xl mx-auto w-full px-6 lg:px-8 pt-8">
          <motion.div
            initial={{ opacity: 0, y: -20 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.6 }}
            className="flex items-center justify-between"
          >
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl flex items-center justify-center font-bold text-base pulse-glow" style={{ background: "linear-gradient(135deg, var(--accent-green), var(--accent-green-dim))", color: "#000", fontFamily: "var(--font-display)" }}>
                M
              </div>
              <span className="text-xl font-bold tracking-wide" style={{ fontFamily: "var(--font-display)" }}>
                MRR <span style={{ color: "var(--accent-green)" }}>FANTASY</span>
              </span>
            </div>
            <div className="flex items-center gap-3">
              <Link href="/login" className="btn-secondary text-xs no-underline py-2.5 px-5">
                Login
              </Link>
              <Link href="/register" className="btn-primary text-xs no-underline py-2.5 px-5">
                Get Started
              </Link>
            </div>
          </motion.div>
        </div>

        {/* Hero content */}
        <div className="relative z-10 flex-1 flex items-center justify-center">
          <div className="max-w-5xl mx-auto px-6 lg:px-8 text-center">
            <motion.div
              initial={{ opacity: 0, y: 50 }}
              animate={{ opacity: 1, y: 0 }}
              transition={{ duration: 1, delay: 0.2, ease: [0.25, 0.46, 0.45, 0.94] }}
            >
              <div className="inline-flex items-center gap-2 px-5 py-2.5 rounded-full mb-10" style={{ background: "rgba(0, 230, 118, 0.06)", border: "1px solid rgba(0, 230, 118, 0.12)" }}>
                <Star size={13} style={{ color: "var(--accent-amber)" }} />
                <span className="text-xs uppercase tracking-[0.15em] font-medium" style={{ color: "var(--accent-green)", fontFamily: "var(--font-display)" }}>Season 2026 is Live</span>
              </div>

              <h1 className="text-6xl sm:text-8xl lg:text-9xl font-extrabold leading-[0.9] mb-8" style={{ fontFamily: "var(--font-display)" }}>
                <span className="block">YOUR PITCH</span>
                <span className="block text-glow-green" style={{ color: "var(--accent-green)" }}>YOUR RULES</span>
              </h1>

              <p className="text-lg sm:text-xl max-w-2xl mx-auto mb-14 leading-relaxed" style={{ color: "var(--text-muted)", fontFamily: "var(--font-body)" }}>
                Draft 6 world-class footballers. Set your formation.
                Pick your captain. Dominate your league.
              </p>

              <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
                <Link href="/register" className="btn-primary text-base flex items-center gap-2 no-underline px-10 py-4">
                  Start Your Journey
                  <ChevronRight size={18} />
                </Link>
                <Link href="/login" className="btn-secondary text-base no-underline px-10 py-4">
                  I Have an Account
                </Link>
              </div>
            </motion.div>
          </div>
        </div>

        {/* Stats bar */}
        <motion.div
          initial={{ opacity: 0, y: 30 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ delay: 0.8, duration: 0.7 }}
          className="relative z-10 pb-20"
        >
          <div className="max-w-3xl mx-auto px-6 grid grid-cols-3 gap-8">
            {[
              { value: "30+", label: "Players" },
              { value: "6", label: "Per Squad" },
              { value: "∞", label: "Glory" },
            ].map((stat, i) => (
              <div key={i} className="text-center">
                <div className="text-4xl sm:text-5xl font-extrabold mb-1" style={{ fontFamily: "var(--font-display)", color: "var(--accent-amber)" }}>
                  {stat.value}
                </div>
                <div className="text-[11px] uppercase tracking-[0.2em] font-medium" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>
                  {stat.label}
                </div>
              </div>
            ))}
          </div>
        </motion.div>
      </section>

      {/* ── FEATURES SECTION ── */}
      <section className="relative py-28" style={{ background: "var(--bg-secondary)" }}>
        <div className="max-w-7xl mx-auto px-6 lg:px-8">
          <motion.div
            initial={{ opacity: 0 }}
            whileInView={{ opacity: 1 }}
            viewport={{ once: true }}
            className="text-center mb-20"
          >
            <span className="text-xs uppercase tracking-[0.2em] font-medium mb-4 block" style={{ color: "var(--accent-green)", fontFamily: "var(--font-display)" }}>
              The Game
            </span>
            <h2 className="text-4xl sm:text-5xl font-extrabold" style={{ fontFamily: "var(--font-display)" }}>
              HOW IT <span style={{ color: "var(--accent-green)" }}>WORKS</span>
            </h2>
          </motion.div>

          <motion.div
            variants={containerVariants}
            initial="hidden"
            whileInView="visible"
            viewport={{ once: true }}
            className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-4 gap-6"
          >
            {features.map((feature, i) => {
              const Icon = feature.icon;
              return (
                <motion.div key={i} variants={itemVariants} className="glass-card p-8 group relative overflow-hidden">
                  <div className="absolute top-0 right-0 w-32 h-32 rounded-full opacity-[0.04] -translate-y-1/2 translate-x-1/2" style={{ background: `radial-gradient(circle, ${feature.accent}, transparent 70%)` }} />
                  <div className="relative">
                    <div className="w-14 h-14 rounded-2xl flex items-center justify-center mb-6 transition-transform group-hover:scale-110" style={{ background: `${feature.accent}10`, border: `1px solid ${feature.accent}20` }}>
                      <Icon size={26} style={{ color: feature.accent }} />
                    </div>
                    <h3 className="text-lg font-bold mb-3" style={{ fontFamily: "var(--font-display)" }}>{feature.title}</h3>
                    <p className="text-sm leading-relaxed" style={{ color: "var(--text-muted)" }}>{feature.description}</p>
                  </div>
                </motion.div>
              );
            })}
          </motion.div>
        </div>
      </section>

      {/* ── SCORING TABLE ── */}
      <section className="py-28 pitch-pattern">
        <div className="max-w-4xl mx-auto px-6 lg:px-8">
          <motion.div
            initial={{ opacity: 0 }}
            whileInView={{ opacity: 1 }}
            viewport={{ once: true }}
            className="text-center mb-16"
          >
            <span className="text-xs uppercase tracking-[0.2em] font-medium mb-4 block" style={{ color: "var(--accent-amber)", fontFamily: "var(--font-display)" }}>
              Points
            </span>
            <h2 className="text-4xl sm:text-5xl font-extrabold" style={{ fontFamily: "var(--font-display)" }}>
              SCORING <span style={{ color: "var(--accent-amber)" }}>SYSTEM</span>
            </h2>
          </motion.div>

          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            className="glass-card overflow-hidden"
          >
            <table className="w-full text-left">
              <thead>
                <tr style={{ borderBottom: "1px solid rgba(255,255,255,0.06)" }}>
                  <th className="px-8 py-5 text-[11px] uppercase tracking-[0.15em] font-semibold" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>Action</th>
                  <th className="px-8 py-5 text-[11px] uppercase tracking-[0.15em] font-semibold text-right" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>Points</th>
                </tr>
              </thead>
              <tbody>
                {[
                  { action: "Goal (Forward)", points: "+10" },
                  { action: "Goal (Midfielder)", points: "+8" },
                  { action: "Goal (Defender/GK)", points: "+12" },
                  { action: "Assist", points: "+5" },
                  { action: "Clean Sheet (DEF/GK)", points: "+6" },
                  { action: "Save (GK)", points: "+2" },
                  { action: "Tackle Won", points: "+2" },
                ].map((row, i) => (
                  <tr key={i} className="transition-colors hover:bg-white/[0.02]" style={{ borderBottom: "1px solid rgba(255,255,255,0.04)" }}>
                    <td className="px-8 py-4 text-sm font-medium" style={{ color: "var(--text-secondary)" }}>{row.action}</td>
                    <td className="px-8 py-4 text-right font-bold text-base" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>{row.points}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </motion.div>
        </div>
      </section>

      {/* ── CTA ── */}
      <section className="py-32 relative overflow-hidden" style={{ background: "var(--bg-secondary)" }}>
        <div className="absolute inset-0 pointer-events-none">
          <div className="absolute w-[800px] h-[800px] rounded-full opacity-[0.03] top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2" style={{ background: "radial-gradient(circle, #00e676, transparent 60%)" }} />
        </div>
        <motion.div
          initial={{ opacity: 0, y: 30 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          className="relative z-10 max-w-3xl mx-auto text-center px-6"
        >
          <Hexagon size={48} className="mx-auto mb-8 opacity-20" style={{ color: "var(--accent-green)" }} />
          <h2 className="text-5xl sm:text-6xl font-extrabold mb-6" style={{ fontFamily: "var(--font-display)" }}>
            READY TO{" "}
            <span className="text-glow-green" style={{ color: "var(--accent-green)" }}>MANAGE</span>?
          </h2>
          <p className="mb-10 text-lg" style={{ color: "var(--text-muted)" }}>
            Create your free account and build your dream squad today.
          </p>
          <Link href="/register" className="btn-primary text-lg inline-flex items-center gap-3 no-underline px-12 py-5">
            Create Your Team
            <ChevronRight size={20} />
          </Link>
        </motion.div>
      </section>

      {/* ── FOOTER ── */}
      <footer className="py-10 text-center" style={{ borderTop: "1px solid rgba(255,255,255,0.04)" }}>
        <p className="text-sm font-medium" style={{ color: "var(--text-muted)", fontFamily: "var(--font-body)" }}>
          &copy; 2026 MRR Fantasy. All rights reserved.
        </p>
      </footer>
    </div>
  );
}
