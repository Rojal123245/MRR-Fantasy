"use client";

import Link from "next/link";
import { motion } from "framer-motion";
import { Trophy, Users, Shield, Zap, ChevronRight, Star } from "lucide-react";

const features = [
  {
    icon: Shield,
    title: "Build Your Squad",
    description: "Pick 6 elite players across all positions to form your ultimate fantasy team.",
  },
  {
    icon: Zap,
    title: "Earn Points",
    description: "Score points for goals, assists, clean sheets, saves, and tackles every match week.",
  },
  {
    icon: Users,
    title: "Create Leagues",
    description: "Compete head-to-head with friends in private leagues with invite codes.",
  },
  {
    icon: Trophy,
    title: "Climb the Ranks",
    description: "Track your standing on live leaderboards and prove you're the best manager.",
  },
];

const containerVariants = {
  hidden: { opacity: 0 },
  visible: {
    opacity: 1,
    transition: { staggerChildren: 0.15 },
  },
};

const itemVariants = {
  hidden: { opacity: 0, y: 30 },
  visible: { opacity: 1, y: 0, transition: { duration: 0.6, ease: "easeOut" as const } },
};

export default function LandingPage() {
  return (
    <div className="min-h-screen pitch-pattern">
      {/* Hero Section */}
      <section className="hero-gradient floodlight relative overflow-hidden">
        {/* Decorative elements */}
        <div className="absolute top-0 left-0 w-full h-full">
          <div className="absolute top-20 left-10 w-2 h-2 rounded-full bg-[var(--accent-green)] opacity-30 animate-pulse" />
          <div className="absolute top-40 right-20 w-3 h-3 rounded-full bg-[var(--accent-amber)] opacity-20 animate-pulse" style={{ animationDelay: "1s" }} />
          <div className="absolute bottom-40 left-1/4 w-1.5 h-1.5 rounded-full bg-[var(--accent-green)] opacity-25 animate-pulse" style={{ animationDelay: "2s" }} />
        </div>

        <div className="max-w-6xl mx-auto px-4 sm:px-6 lg:px-8 pt-20 pb-32 relative">
          {/* Top bar */}
          <motion.div
            initial={{ opacity: 0, y: -20 }}
            animate={{ opacity: 1, y: 0 }}
            className="flex items-center justify-between mb-20"
          >
            <div className="flex items-center gap-3">
              <div className="w-10 h-10 rounded-xl flex items-center justify-center font-bold text-base" style={{ background: "linear-gradient(135deg, var(--accent-green), var(--accent-green-dim))", color: "var(--bg-primary)", fontFamily: "var(--font-display)" }}>
                M
              </div>
              <span className="text-xl font-bold tracking-wider" style={{ fontFamily: "var(--font-display)" }}>
                MRR <span style={{ color: "var(--accent-green)" }}>FANTASY</span>
              </span>
            </div>
            <div className="flex items-center gap-3">
              <Link href="/login" className="btn-secondary text-sm no-underline">
                Login
              </Link>
              <Link href="/register" className="btn-primary text-sm no-underline">
                Get Started
              </Link>
            </div>
          </motion.div>

          {/* Hero content */}
          <motion.div
            initial={{ opacity: 0, y: 40 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ duration: 0.8, delay: 0.2 }}
            className="text-center max-w-4xl mx-auto"
          >
            <div className="inline-flex items-center gap-2 px-4 py-2 rounded-full mb-8" style={{ background: "rgba(0, 230, 118, 0.08)", border: "1px solid rgba(0, 230, 118, 0.2)" }}>
              <Star size={14} style={{ color: "var(--accent-amber)" }} />
              <span className="text-xs uppercase tracking-wider" style={{ color: "var(--accent-green)", fontFamily: "var(--font-display)" }}>Season 2026 is Live</span>
            </div>

            <h1 className="text-5xl sm:text-7xl lg:text-8xl font-bold mb-6 leading-tight" style={{ fontFamily: "var(--font-display)" }}>
              YOUR PITCH.
              <br />
              <span className="text-glow-green" style={{ color: "var(--accent-green)" }}>YOUR RULES.</span>
            </h1>

            <p className="text-lg sm:text-xl mb-12 max-w-2xl mx-auto" style={{ color: "var(--text-muted)", fontFamily: "var(--font-body)", lineHeight: 1.7 }}>
              Draft 6 world-class footballers, compete in leagues with friends,
              and watch your points pile up every match week. The ultimate fantasy
              football experience awaits.
            </p>

            <div className="flex flex-col sm:flex-row items-center justify-center gap-4">
              <Link href="/register" className="btn-primary text-base flex items-center gap-2 no-underline px-8 py-4">
                Start Your Journey
                <ChevronRight size={18} />
              </Link>
              <Link href="/login" className="btn-secondary text-base no-underline px-8 py-4">
                I Have an Account
              </Link>
            </div>
          </motion.div>

          {/* Stats bar */}
          <motion.div
            initial={{ opacity: 0, y: 30 }}
            animate={{ opacity: 1, y: 0 }}
            transition={{ delay: 0.6, duration: 0.6 }}
            className="mt-24 grid grid-cols-3 gap-8 max-w-2xl mx-auto"
          >
            {[
              { value: "30+", label: "Players" },
              { value: "6", label: "Per Squad" },
              { value: "∞", label: "Glory" },
            ].map((stat, i) => (
              <div key={i} className="text-center">
                <div className="text-3xl sm:text-4xl font-bold mb-1" style={{ fontFamily: "var(--font-display)", color: "var(--accent-amber)" }}>
                  {stat.value}
                </div>
                <div className="text-xs uppercase tracking-wider" style={{ color: "var(--text-muted)", fontFamily: "var(--font-display)" }}>
                  {stat.label}
                </div>
              </div>
            ))}
          </motion.div>
        </div>
      </section>

      {/* Features Section */}
      <section className="py-24" style={{ background: "var(--bg-secondary)" }}>
        <div className="max-w-6xl mx-auto px-4 sm:px-6 lg:px-8">
          <motion.div
            initial={{ opacity: 0 }}
            whileInView={{ opacity: 1 }}
            viewport={{ once: true }}
            className="text-center mb-16"
          >
            <h2 className="text-3xl sm:text-4xl font-bold mb-4" style={{ fontFamily: "var(--font-display)" }}>
              HOW IT <span style={{ color: "var(--accent-green)" }}>WORKS</span>
            </h2>
            <p style={{ color: "var(--text-muted)" }}>Everything you need to dominate the fantasy league</p>
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
                <motion.div key={i} variants={itemVariants} className="glass-card p-6 text-center">
                  <div className="w-14 h-14 rounded-2xl flex items-center justify-center mx-auto mb-4" style={{ background: "rgba(0, 230, 118, 0.1)" }}>
                    <Icon size={28} style={{ color: "var(--accent-green)" }} />
                  </div>
                  <h3 className="text-lg font-bold mb-2" style={{ fontFamily: "var(--font-display)" }}>{feature.title}</h3>
                  <p className="text-sm" style={{ color: "var(--text-muted)", lineHeight: 1.6 }}>{feature.description}</p>
                </motion.div>
              );
            })}
          </motion.div>
        </div>
      </section>

      {/* Points breakdown */}
      <section className="py-24">
        <div className="max-w-4xl mx-auto px-4 sm:px-6 lg:px-8">
          <motion.div
            initial={{ opacity: 0 }}
            whileInView={{ opacity: 1 }}
            viewport={{ once: true }}
            className="text-center mb-16"
          >
            <h2 className="text-3xl sm:text-4xl font-bold mb-4" style={{ fontFamily: "var(--font-display)" }}>
              SCORING <span style={{ color: "var(--accent-amber)" }}>SYSTEM</span>
            </h2>
            <p style={{ color: "var(--text-muted)" }}>How your players earn you points</p>
          </motion.div>

          <motion.div
            initial={{ opacity: 0, y: 20 }}
            whileInView={{ opacity: 1, y: 0 }}
            viewport={{ once: true }}
            className="glass-card overflow-hidden"
          >
            <table className="w-full text-left">
              <thead>
                <tr style={{ borderBottom: "1px solid var(--border-color)" }}>
                  <th className="px-6 py-4 text-sm uppercase tracking-wider" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>Action</th>
                  <th className="px-6 py-4 text-sm uppercase tracking-wider text-right" style={{ fontFamily: "var(--font-display)", color: "var(--text-muted)" }}>Points</th>
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
                  <tr key={i} style={{ borderBottom: "1px solid var(--border-color)" }}>
                    <td className="px-6 py-4 text-sm" style={{ color: "var(--text-secondary)" }}>{row.action}</td>
                    <td className="px-6 py-4 text-right font-bold" style={{ fontFamily: "var(--font-display)", color: "var(--accent-green)" }}>{row.points}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          </motion.div>
        </div>
      </section>

      {/* CTA */}
      <section className="py-24" style={{ background: "var(--bg-secondary)" }}>
        <motion.div
          initial={{ opacity: 0, y: 30 }}
          whileInView={{ opacity: 1, y: 0 }}
          viewport={{ once: true }}
          className="max-w-3xl mx-auto text-center px-4"
        >
          <h2 className="text-4xl sm:text-5xl font-bold mb-6" style={{ fontFamily: "var(--font-display)" }}>
            READY TO <span className="text-glow-green" style={{ color: "var(--accent-green)" }}>MANAGE</span>?
          </h2>
          <p className="mb-8" style={{ color: "var(--text-muted)", fontSize: "1.1rem" }}>
            Create your free account and build your dream squad today.
          </p>
          <Link href="/register" className="btn-primary text-lg inline-flex items-center gap-2 no-underline px-10 py-4">
            Create Your Team
            <ChevronRight size={20} />
          </Link>
        </motion.div>
      </section>

      {/* Footer */}
      <footer className="py-8 text-center" style={{ borderTop: "1px solid var(--border-color)" }}>
        <p className="text-sm" style={{ color: "var(--text-muted)" }}>
          &copy; 2026 MRR Fantasy. All rights reserved.
        </p>
      </footer>
    </div>
  );
}
