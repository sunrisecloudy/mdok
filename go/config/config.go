// Package config loads mdok.toml and merges CLI overrides.
package config

import (
	"os"
	"path/filepath"
	"time"

	"github.com/BurntSushi/toml"

	"mdok/core"
)

type executionConfig struct {
	AllowedSchemes []string      `toml:"allowed_schemes"`
	ConnectTimeout time.Duration `toml:"connect_timeout"`
	TotalTimeout   time.Duration `toml:"total_timeout"`
}

type policyConfig struct {
	AllowedHosts     []string `toml:"allowed_hosts"`
	AllowedReadPaths []string `toml:"allowed_read_paths"`
}

type fileConfig struct {
	Language   string          `toml:"language"`
	CurlCompat string          `toml:"curl_compat"`
	Execution  executionConfig `toml:"execution"`
	Policy     policyConfig    `toml:"policy"`
}

// Load reads mdok.toml (if present) and applies CLI allow-host additions.
// Missing file yields defaults mirroring the Rust CLI: http/https schemes,
// 10s connect timeout, 300s total timeout.
func Load(path string, allowHosts []string) (*core.ExecConfig, error) {
	file := fileConfig{}
	if path != "" {
		data, err := os.ReadFile(path)
		if err != nil {
			return nil, err
		}
		if err := toml.Unmarshal(data, &file); err != nil {
			return nil, err
		}
	}
	cfg := &core.ExecConfig{
		AllowedHosts:     append(append([]string{}, file.Policy.AllowedHosts...), allowHosts...),
		AllowedSchemes:   file.Execution.AllowedSchemes,
		AllowedReadPaths: file.Policy.AllowedReadPaths,
		ConnectTimeout:   file.Execution.ConnectTimeout,
		TotalTimeout:     file.Execution.TotalTimeout,
	}
	if len(cfg.AllowedSchemes) == 0 {
		cfg.AllowedSchemes = []string{"http", "https"}
	}
	if cfg.ConnectTimeout == 0 {
		cfg.ConnectTimeout = 10 * time.Second
	}
	if cfg.TotalTimeout == 0 {
		cfg.TotalTimeout = 300 * time.Second
	}
	for i, p := range cfg.AllowedReadPaths {
		if abs, err := filepath.Abs(p); err == nil {
			cfg.AllowedReadPaths[i] = abs
		}
	}
	return cfg, nil
}
