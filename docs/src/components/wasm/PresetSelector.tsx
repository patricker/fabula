import React, { useState } from 'react';
import { PRESET_GROUPS, type Preset } from './presets';
import styles from './PresetSelector.module.css';

interface PresetSelectorProps {
  onSelect: (preset: Preset) => void;
}

export default function PresetSelector({ onSelect }: PresetSelectorProps) {
  const [description, setDescription] = useState<string | null>(null);

  return (
    <div className={styles.selector}>
      <div className={styles.row}>
        <label className={styles.label}>Examples</label>
        <select
          className={styles.select}
          onChange={(e) => {
            const idx = parseInt(e.target.value, 10);
            if (!isNaN(idx)) {
              const all = PRESET_GROUPS.flatMap(g => g.presets);
              const preset = all[idx];
              setDescription(preset.description);
              onSelect(preset);
            }
          }}
          defaultValue=""
        >
          <option value="" disabled>Choose an example...</option>
          {(() => {
            let idx = 0;
            return PRESET_GROUPS.map((group) => (
              <optgroup key={group.label} label={group.label}>
                {group.presets.map((preset) => {
                  const i = idx++;
                  return (
                    <option key={i} value={i}>
                      {preset.label}
                    </option>
                  );
                })}
              </optgroup>
            ));
          })()}
        </select>
      </div>
      {description && (
        <div className={styles.description}>{description}</div>
      )}
    </div>
  );
}
