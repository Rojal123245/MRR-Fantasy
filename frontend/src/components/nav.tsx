"use client";

import Link from "next/link";
import { usePathname, useRouter } from "next/navigation";
import { useEffect, useState } from "react";
import { motion } from "framer-motion";
import { Trophy, Users, Shield, LayoutDashboard, LogOut, Menu, X } from "lucide-react";
import { getUser, clearAuth, isAuthenticated } from "@/lib/auth";
import type { User } from "@/lib/api";

const navLinks = [
  { href: "/dashboard", label: "Dashboard", icon: LayoutDashboard },
  { href: "/team", label: "My Team", icon: Shield },
  { href: "/league", label: "Leagues", icon: Users },
  { href: "/leaderboard", label: "Leaderboard", icon: Trophy },
];

export default function Nav() {
  const pathname = usePathname();
  const router = useRouter();
  const [user, setUser] = useState<User | null>(null);
  const [mobileOpen, setMobileOpen] = useState(false);

  useEffect(() => {
    if (isAuthenticated()) {
      setUser(getUser());
    }
  }, []);

  const handleLogout = () => {
    clearAuth();
    router.push("/");
  };

  if (!user) return null;

  return (
    <nav className="fixed top-0 left-0 right-0 z-50" style={{ background: "rgba(10, 14, 23, 0.92)", backdropFilter: "blur(20px)", borderBottom: "1px solid var(--border-color)" }}>
      <div className="max-w-7xl mx-auto px-4 sm:px-6 lg:px-8">
        <div className="flex items-center justify-between h-16">
          {/* Logo */}
          <Link href="/dashboard" className="flex items-center gap-2 no-underline">
            <div className="w-8 h-8 rounded-lg flex items-center justify-center font-bold text-sm" style={{ background: "linear-gradient(135deg, var(--accent-green), var(--accent-green-dim))", color: "var(--bg-primary)", fontFamily: "var(--font-display)" }}>
              M
            </div>
            <span className="text-lg font-bold tracking-wider" style={{ fontFamily: "var(--font-display)", color: "var(--text-primary)" }}>
              MRR <span style={{ color: "var(--accent-green)" }}>FANTASY</span>
            </span>
          </Link>

          {/* Desktop Nav */}
          <div className="hidden md:flex items-center gap-1">
            {navLinks.map((link) => {
              const isActive = pathname === link.href;
              const Icon = link.icon;
              return (
                <Link
                  key={link.href}
                  href={link.href}
                  className="relative flex items-center gap-2 px-4 py-2 rounded-lg text-sm font-medium no-underline transition-colors"
                  style={{
                    fontFamily: "var(--font-body)",
                    color: isActive ? "var(--accent-green)" : "var(--text-muted)",
                  }}
                >
                  <Icon size={16} />
                  {link.label}
                  {isActive && (
                    <motion.div
                      layoutId="activeTab"
                      className="absolute inset-0 rounded-lg"
                      style={{ background: "rgba(0, 230, 118, 0.08)", border: "1px solid rgba(0, 230, 118, 0.15)" }}
                      transition={{ type: "spring", stiffness: 380, damping: 30 }}
                    />
                  )}
                </Link>
              );
            })}
          </div>

          {/* User section */}
          <div className="hidden md:flex items-center gap-4">
            <span className="text-sm" style={{ color: "var(--text-muted)" }}>
              {user.username}
            </span>
            <button onClick={handleLogout} className="flex items-center gap-1 text-sm cursor-pointer bg-transparent border-none" style={{ color: "var(--text-muted)" }} title="Logout">
              <LogOut size={16} />
            </button>
          </div>

          {/* Mobile hamburger */}
          <button className="md:hidden bg-transparent border-none cursor-pointer" style={{ color: "var(--text-primary)" }} onClick={() => setMobileOpen(!mobileOpen)}>
            {mobileOpen ? <X size={24} /> : <Menu size={24} />}
          </button>
        </div>
      </div>

      {/* Mobile menu */}
      {mobileOpen && (
        <motion.div
          initial={{ opacity: 0, y: -10 }}
          animate={{ opacity: 1, y: 0 }}
          className="md:hidden px-4 pb-4"
          style={{ background: "var(--bg-secondary)", borderBottom: "1px solid var(--border-color)" }}
        >
          {navLinks.map((link) => {
            const Icon = link.icon;
            const isActive = pathname === link.href;
            return (
              <Link
                key={link.href}
                href={link.href}
                onClick={() => setMobileOpen(false)}
                className="flex items-center gap-3 px-4 py-3 rounded-lg text-sm no-underline"
                style={{ color: isActive ? "var(--accent-green)" : "var(--text-muted)" }}
              >
                <Icon size={18} />
                {link.label}
              </Link>
            );
          })}
          <button onClick={handleLogout} className="flex items-center gap-3 px-4 py-3 rounded-lg text-sm cursor-pointer bg-transparent border-none w-full" style={{ color: "var(--danger)" }}>
            <LogOut size={18} />
            Logout
          </button>
        </motion.div>
      )}
    </nav>
  );
}
