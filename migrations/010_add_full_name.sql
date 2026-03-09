-- Add full_name column to users for real-name identification.
ALTER TABLE users ADD COLUMN full_name VARCHAR(100) NOT NULL DEFAULT '';
