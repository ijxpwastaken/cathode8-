use super::{
    FLAG_BREAK, FLAG_CARRY, FLAG_DECIMAL, FLAG_INTERRUPT, FLAG_NEGATIVE, FLAG_OVERFLOW,
    FLAG_UNUSED, FLAG_ZERO, Nes,
};

impl Nes {
    pub(crate) fn step_cpu(&mut self) -> u32 {
        self.cpu_step_ticked_cycles = 0;
        self.cpu_step_in_progress = false;

        if self.dma_cycles > 0 {
            self.dma_cycles -= 1;
            self.total_cycles += 1;
            return 1;
        }

        self.cpu_step_in_progress = true;

        if self.pending_nmi {
            self.pending_nmi = false;
            self.service_nmi();
            self.total_cycles += 7;
            self.cpu_step_in_progress = false;
            return 7;
        }

        if self.pending_irq && !self.get_flag(FLAG_INTERRUPT) {
            self.pending_irq = false;
            self.service_irq();
            self.total_cycles += 7;
            self.cpu_step_in_progress = false;
            return 7;
        }

        let opcode_pc = self.pc;
        let opcode = self.fetch_byte();

        match opcode {
            0x8A => {
                self.a = self.x;
                self.update_zn(self.a);
                self.total_cycles += 2;
                self.cpu_step_in_progress = false;
                return 2;
            }
            0x9A => {
                self.sp = self.x;
                self.total_cycles += 2;
                self.cpu_step_in_progress = false;
                return 2;
            }
            0xAA => {
                self.x = self.a;
                self.update_zn(self.x);
                self.total_cycles += 2;
                self.cpu_step_in_progress = false;
                return 2;
            }
            0xBA => {
                self.x = self.sp;
                self.update_zn(self.x);
                self.total_cycles += 2;
                self.cpu_step_in_progress = false;
                return 2;
            }
            0xCA => {
                self.x = self.x.wrapping_sub(1);
                self.update_zn(self.x);
                self.total_cycles += 2;
                self.cpu_step_in_progress = false;
                return 2;
            }
            0xEA => {
                self.total_cycles += 2;
                self.cpu_step_in_progress = false;
                return 2;
            }
            _ => {}
        }

        // Two-byte unofficial NOPs used by test ROMs for timing.
        if matches!(opcode, 0x80 | 0x82 | 0x89 | 0xC2 | 0xE2) {
            self.fetch_byte();
            self.total_cycles += 2;
            self.cpu_step_in_progress = false;
            return 2;
        }

        if let Some(cycles) = self.exec_unofficial(opcode, opcode_pc) {
            self.total_cycles += cycles as u64;
            self.cpu_step_in_progress = false;
            return cycles;
        }

        let cc = opcode & 0x03;
        let aaa = opcode >> 5;
        let bbb = (opcode >> 2) & 0x07;

        let cycles = match cc {
            0x01 => self.exec_group1(opcode, aaa, bbb, opcode_pc),
            0x02 => self.exec_group2(opcode, aaa, bbb, opcode_pc),
            0x03 => {
                self.note_unknown_opcode(opcode, opcode_pc);
                2
            }
            _ => self.exec_group0(opcode, opcode_pc),
        };

        self.total_cycles += cycles as u64;
        self.cpu_step_in_progress = false;
        cycles
    }

    fn exec_group1(&mut self, opcode: u8, aaa: u8, bbb: u8, opcode_pc: u16) -> u32 {
        let is_store = aaa == 4;

        if bbb == 2 {
            if is_store {
                return 2;
            }
            let value = self.fetch_byte();
            self.exec_group1_alu(aaa, value);
            return 2;
        }

        let (addr, base, page_crossed, mut cycles) = match bbb {
            0 => (self.addr_indx(), 0, false, 6),
            1 => (self.addr_zp(), 0, false, 3),
            3 => (self.addr_abs(), 0, false, 4),
            4 => {
                let (addr, page, base) = self.addr_indy_with_base();
                (addr, base, page, 5)
            }
            5 => (self.addr_zpx(), 0, false, 4),
            6 => {
                let (addr, page, base) = self.addr_absy_with_base();
                (addr, base, page, 4)
            }
            7 => {
                let (addr, page, base) = self.addr_absx_with_base();
                (addr, base, page, 4)
            }
            _ => {
                self.note_unknown_opcode(opcode, opcode_pc);
                return 2;
            }
        };

        if is_store {
            // Indexed store instructions perform a dummy read before the write.
            if matches!(bbb, 4 | 6 | 7) {
                let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                let _ = self.cpu_read(dummy_addr);
            }
            let value = self.a;
            self.cpu_write(addr, value);
            return match bbb {
                4 => 6,
                6 | 7 => 5,
                _ => cycles,
            };
        }

        if page_crossed && matches!(bbb, 4 | 6 | 7) {
            let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
            let _ = self.cpu_read(dummy_addr);
            cycles += 1;
        }

        let value = self.cpu_read(addr);
        self.exec_group1_alu(aaa, value);

        cycles
    }

    fn exec_group1_alu(&mut self, aaa: u8, value: u8) {
        match aaa {
            0 => self.ora(value),
            1 => self.and(value),
            2 => self.eor(value),
            3 => self.adc(value),
            4 => {}
            5 => {
                self.a = value;
                self.update_zn(self.a);
            }
            6 => self.compare(self.a, value),
            7 => self.sbc(value),
            _ => {}
        }
    }

    fn exec_group2(&mut self, opcode: u8, aaa: u8, bbb: u8, opcode_pc: u16) -> u32 {
        match aaa {
            4 => self.exec_stx(bbb),
            5 => self.exec_ldx(bbb),
            6 => self.exec_rmw(bbb, RmwOp::Dec),
            7 => self.exec_rmw(bbb, RmwOp::Inc),
            0 => self.exec_rmw(bbb, RmwOp::Asl),
            1 => self.exec_rmw(bbb, RmwOp::Rol),
            2 => self.exec_rmw(bbb, RmwOp::Lsr),
            3 => self.exec_rmw(bbb, RmwOp::Ror),
            _ => {
                if matches!(opcode, 0x9A | 0xBA) {
                    self.exec_group0(opcode, opcode_pc)
                } else {
                    self.note_unknown_opcode(opcode, opcode_pc);
                    2
                }
            }
        }
    }

    fn exec_stx(&mut self, bbb: u8) -> u32 {
        match bbb {
            1 => {
                let addr = self.addr_zp();
                self.cpu_write(addr, self.x);
                3
            }
            3 => {
                let addr = self.addr_abs();
                self.cpu_write(addr, self.x);
                4
            }
            5 => {
                let addr = self.addr_zpy();
                self.cpu_write(addr, self.x);
                4
            }
            _ => 2,
        }
    }

    fn exec_ldx(&mut self, bbb: u8) -> u32 {
        match bbb {
            0 | 2 => {
                self.x = self.fetch_byte();
                self.update_zn(self.x);
                2
            }
            1 => {
                let addr = self.addr_zp();
                self.x = self.cpu_read(addr);
                self.update_zn(self.x);
                3
            }
            3 => {
                let addr = self.addr_abs();
                self.x = self.cpu_read(addr);
                self.update_zn(self.x);
                4
            }
            5 => {
                let addr = self.addr_zpy();
                self.x = self.cpu_read(addr);
                self.update_zn(self.x);
                4
            }
            7 => {
                let (addr, page, base) = self.addr_absy_with_base();
                if page {
                    let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                    let _ = self.cpu_read(dummy_addr);
                }
                self.x = self.cpu_read(addr);
                self.update_zn(self.x);
                4 + page as u32
            }
            _ => 2,
        }
    }

    fn exec_rmw(&mut self, bbb: u8, op: RmwOp) -> u32 {
        if bbb == 2 {
            if matches!(op, RmwOp::Dec | RmwOp::Inc) {
                return 2;
            }
            self.a = self.apply_rmw(op, self.a);
            return 2;
        }

        let (addr, cycles, indexed_base) = match bbb {
            1 => (self.addr_zp(), 5, None),
            3 => (self.addr_abs(), 6, None),
            5 => (self.addr_zpx(), 6, None),
            7 => {
                let (addr, _page, base) = self.addr_absx_with_base();
                (addr, 7, Some(base))
            }
            _ => return 2,
        };

        if let Some(base) = indexed_base {
            let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
            let _ = self.cpu_read(dummy_addr);
        }

        let value = self.cpu_read(addr);
        self.cpu_write(addr, value);
        let out = self.apply_rmw(op, value);
        self.cpu_write(addr, out);
        cycles
    }

    fn apply_rmw(&mut self, op: RmwOp, value: u8) -> u8 {
        match op {
            RmwOp::Asl => self.asl(value),
            RmwOp::Rol => self.rol(value),
            RmwOp::Lsr => self.lsr(value),
            RmwOp::Ror => self.ror(value),
            RmwOp::Dec => {
                let out = value.wrapping_sub(1);
                self.update_zn(out);
                out
            }
            RmwOp::Inc => {
                let out = value.wrapping_add(1);
                self.update_zn(out);
                out
            }
        }
    }

    fn exec_unofficial(&mut self, opcode: u8, _opcode_pc: u16) -> Option<u32> {
        match opcode {
            // SHA / AHX (indirect),Y
            0x93 => {
                let (addr, page, base) = self.addr_indy_with_base();
                let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                let _ = self.cpu_read(dummy_addr);
                let h = ((base >> 8) as u8).wrapping_add(1);
                let value = self.a & self.x & h;
                let write_addr = if page {
                    let unstable_hi = h & self.x;
                    ((unstable_hi as u16) << 8) | (addr & 0x00FF)
                } else {
                    addr
                };
                self.cpu_write(write_addr, value);
                return Some(6);
            }
            // SHA / AHX absolute,Y
            0x9F => {
                let (addr, page, base) = self.addr_absy_with_base();
                let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                let _ = self.cpu_read(dummy_addr);
                let h = ((base >> 8) as u8).wrapping_add(1);
                let value = self.a & self.x & h;
                let write_addr = if page {
                    let unstable_hi = h & self.x;
                    ((unstable_hi as u16) << 8) | (addr & 0x00FF)
                } else {
                    addr
                };
                self.cpu_write(write_addr, value);
                return Some(5);
            }
            // SHS / TAS absolute,Y
            0x9B => {
                let (addr, page, base) = self.addr_absy_with_base();
                let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                let _ = self.cpu_read(dummy_addr);
                self.sp = self.a & self.x;
                let h = ((base >> 8) as u8).wrapping_add(1);
                let value = self.sp & h;
                let write_addr = if page {
                    let unstable_hi = h & self.x;
                    ((unstable_hi as u16) << 8) | (addr & 0x00FF)
                } else {
                    addr
                };
                self.cpu_write(write_addr, value);
                return Some(5);
            }
            // SHY absolute,X
            0x9C => {
                let (addr, page, base) = self.addr_absx_with_base();
                let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                let _ = self.cpu_read(dummy_addr);
                let h = ((base >> 8) as u8).wrapping_add(1);
                let value = self.y & h;
                let write_addr = if page {
                    ((value as u16) << 8) | (addr & 0x00FF)
                } else {
                    addr
                };
                self.cpu_write(write_addr, value);
                return Some(5);
            }
            // SHX absolute,Y
            0x9E => {
                let (addr, page, base) = self.addr_absy_with_base();
                let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                let _ = self.cpu_read(dummy_addr);
                let h = ((base >> 8) as u8).wrapping_add(1);
                let value = self.x & h;
                let write_addr = if page {
                    ((value as u16) << 8) | (addr & 0x00FF)
                } else {
                    addr
                };
                self.cpu_write(write_addr, value);
                return Some(5);
            }
            // LAE / LAS absolute,Y
            0xBB => {
                let (addr, page, base) = self.addr_absy_with_base();
                if page {
                    let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                    let _ = self.cpu_read(dummy_addr);
                }
                let value = self.cpu_read(addr) & self.sp;
                self.a = value;
                self.x = value;
                self.sp = value;
                self.update_zn(value);
                return Some(4 + page as u32);
            }
            _ => {}
        }

        if (opcode & 0x03) != 0x03 {
            return None;
        }

        let aaa = opcode >> 5;
        let bbb = (opcode >> 2) & 0x07;

        if bbb == 2 {
            let imm = self.fetch_byte();
            match aaa {
                // ANC
                0 | 1 => {
                    self.a &= imm;
                    self.update_zn(self.a);
                    self.set_flag(FLAG_CARRY, (self.a & 0x80) != 0);
                    return Some(2);
                }
                // ASR / ALR
                2 => {
                    self.a &= imm;
                    self.a = self.lsr(self.a);
                    return Some(2);
                }
                // ARR
                3 => {
                    self.a &= imm;
                    let carry_in = if self.get_flag(FLAG_CARRY) { 0x80 } else { 0 };
                    self.a = (self.a >> 1) | carry_in;
                    self.update_zn(self.a);
                    self.set_flag(FLAG_CARRY, (self.a & 0x40) != 0);
                    self.set_flag(
                        FLAG_OVERFLOW,
                        (((self.a >> 6) & 0x01) ^ ((self.a >> 5) & 0x01)) != 0,
                    );
                    return Some(2);
                }
                // ANE / XAA (unstable, RP2A03-friendly approximation)
                4 => {
                    self.a = (self.a | 0xEE) & self.x & imm;
                    self.update_zn(self.a);
                    return Some(2);
                }
                // LXA / OAL (unstable, RP2A03-friendly approximation)
                5 => {
                    self.a = (self.a | 0xEE) & imm;
                    self.x = self.a;
                    self.update_zn(self.a);
                    return Some(2);
                }
                // AXS / SBX
                6 => {
                    let in_ax = self.a & self.x;
                    self.set_flag(FLAG_CARRY, in_ax >= imm);
                    self.x = in_ax.wrapping_sub(imm);
                    self.update_zn(self.x);
                    return Some(2);
                }
                // SBC immediate (unofficial alias)
                7 => {
                    self.sbc(imm);
                    return Some(2);
                }
                _ => return None,
            }
        }

        match aaa {
            // SLO / RLA / SRE / RRA / DCP / ISC class
            0 | 1 | 2 | 3 | 6 | 7 => {
                let (addr, cycles, indexed_base) = match bbb {
                    0 => (self.addr_indx(), 8, None),
                    1 => (self.addr_zp(), 5, None),
                    3 => (self.addr_abs(), 6, None),
                    4 => {
                        let (addr, _page, base) = self.addr_indy_with_base();
                        (addr, 8, Some(base))
                    }
                    5 => (self.addr_zpx(), 6, None),
                    6 => {
                        let (addr, _page, base) = self.addr_absy_with_base();
                        (addr, 7, Some(base))
                    }
                    7 => {
                        let (addr, _page, base) = self.addr_absx_with_base();
                        (addr, 7, Some(base))
                    }
                    _ => return None,
                };

                if let Some(base) = indexed_base {
                    let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                    let _ = self.cpu_read(dummy_addr);
                }

                let op = match aaa {
                    0 => UnofficialRmwOp::Slo,
                    1 => UnofficialRmwOp::Rla,
                    2 => UnofficialRmwOp::Sre,
                    3 => UnofficialRmwOp::Rra,
                    6 => UnofficialRmwOp::Dcp,
                    7 => UnofficialRmwOp::Isc,
                    _ => return None,
                };
                self.exec_unofficial_rmw(addr, op);
                Some(cycles)
            }
            // SAX
            4 => {
                let addr = match bbb {
                    0 => self.addr_indx(),
                    1 => self.addr_zp(),
                    3 => self.addr_abs(),
                    5 => self.addr_zpy(),
                    _ => return None,
                };
                let value = self.a & self.x;
                self.cpu_write(addr, value);
                let cycles = match bbb {
                    0 => 6,
                    1 => 3,
                    3 => 4,
                    5 => 4,
                    _ => 2,
                };
                Some(cycles)
            }
            // LAX
            5 => {
                let (value, cycles) = match bbb {
                    0 => {
                        let addr = self.addr_indx();
                        (self.cpu_read(addr), 6)
                    }
                    1 => {
                        let addr = self.addr_zp();
                        (self.cpu_read(addr), 3)
                    }
                    3 => {
                        let addr = self.addr_abs();
                        (self.cpu_read(addr), 4)
                    }
                    4 => {
                        let (addr, page, base) = self.addr_indy_with_base();
                        if page {
                            let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                            let _ = self.cpu_read(dummy_addr);
                        }
                        (self.cpu_read(addr), 5 + page as u32)
                    }
                    5 => {
                        let addr = self.addr_zpy();
                        (self.cpu_read(addr), 4)
                    }
                    6 => {
                        let (addr, page, base) = self.addr_absy_with_base();
                        if page {
                            let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                            let _ = self.cpu_read(dummy_addr);
                        }
                        (self.cpu_read(addr), 4 + page as u32)
                    }
                    7 => {
                        let (addr, page, base) = self.addr_absy_with_base();
                        if page {
                            let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                            let _ = self.cpu_read(dummy_addr);
                        }
                        (self.cpu_read(addr), 4 + page as u32)
                    }
                    _ => return None,
                };
                self.a = value;
                self.x = value;
                self.update_zn(value);
                Some(cycles)
            }
            _ => None,
        }
    }

    fn exec_unofficial_rmw(&mut self, addr: u16, op: UnofficialRmwOp) {
        let value = self.cpu_read(addr);
        self.cpu_write(addr, value);

        let out = match op {
            UnofficialRmwOp::Slo => {
                let shifted = self.asl(value);
                self.a |= shifted;
                self.update_zn(self.a);
                shifted
            }
            UnofficialRmwOp::Rla => {
                let shifted = self.rol(value);
                self.a &= shifted;
                self.update_zn(self.a);
                shifted
            }
            UnofficialRmwOp::Sre => {
                let shifted = self.lsr(value);
                self.a ^= shifted;
                self.update_zn(self.a);
                shifted
            }
            UnofficialRmwOp::Rra => {
                let shifted = self.ror(value);
                self.adc(shifted);
                shifted
            }
            UnofficialRmwOp::Dcp => {
                let decremented = value.wrapping_sub(1);
                self.compare(self.a, decremented);
                decremented
            }
            UnofficialRmwOp::Isc => {
                let incremented = value.wrapping_add(1);
                self.sbc(incremented);
                incremented
            }
        };

        self.cpu_write(addr, out);
    }

    fn exec_group0(&mut self, opcode: u8, opcode_pc: u16) -> u32 {
        match opcode {
            0x00 => {
                self.pc = self.pc.wrapping_add(1);
                self.push_u16(self.pc);
                self.push((self.p | FLAG_BREAK) | FLAG_UNUSED);
                self.set_flag(FLAG_INTERRUPT, true);
                self.pc = self.read_u16(0xFFFE);
                7
            }
            0x08 => {
                self.push(self.p | FLAG_BREAK | FLAG_UNUSED);
                3
            }
            0x10 => self.branch(!self.get_flag(FLAG_NEGATIVE)),
            0x18 => {
                self.set_flag(FLAG_CARRY, false);
                2
            }
            0x20 => {
                let addr = self.fetch_word();
                self.push_u16(self.pc.wrapping_sub(1));
                self.pc = addr;
                6
            }
            0x24 => {
                let addr = self.addr_zp();
                let value = self.cpu_read(addr);
                self.bit(value);
                3
            }
            0x28 => {
                self.p = self.pop();
                self.p &= !FLAG_BREAK;
                self.p |= FLAG_UNUSED;
                4
            }
            0x2C => {
                let addr = self.addr_abs();
                let value = self.cpu_read(addr);
                self.bit(value);
                4
            }
            0x30 => self.branch(self.get_flag(FLAG_NEGATIVE)),
            0x38 => {
                self.set_flag(FLAG_CARRY, true);
                2
            }
            0x40 => {
                self.p = self.pop();
                self.p &= !FLAG_BREAK;
                self.p |= FLAG_UNUSED;
                self.pc = self.pop_u16();
                6
            }
            0x48 => {
                self.push(self.a);
                3
            }
            0x4C => {
                self.pc = self.fetch_word();
                3
            }
            0x50 => self.branch(!self.get_flag(FLAG_OVERFLOW)),
            0x58 => {
                self.set_flag(FLAG_INTERRUPT, false);
                2
            }
            0x60 => {
                self.pc = self.pop_u16().wrapping_add(1);
                6
            }
            0x68 => {
                self.a = self.pop();
                self.update_zn(self.a);
                4
            }
            0x6C => {
                let ptr = self.fetch_word();
                self.pc = self.read_u16_bug(ptr);
                5
            }
            0x70 => self.branch(self.get_flag(FLAG_OVERFLOW)),
            0x78 => {
                self.set_flag(FLAG_INTERRUPT, true);
                2
            }
            0x84 => {
                let addr = self.addr_zp();
                self.cpu_write(addr, self.y);
                3
            }
            0x88 => {
                self.y = self.y.wrapping_sub(1);
                self.update_zn(self.y);
                2
            }
            0x8A => {
                self.a = self.x;
                self.update_zn(self.a);
                2
            }
            0x8C => {
                let addr = self.addr_abs();
                self.cpu_write(addr, self.y);
                4
            }
            0x90 => self.branch(!self.get_flag(FLAG_CARRY)),
            0x94 => {
                let addr = self.addr_zpx();
                self.cpu_write(addr, self.y);
                4
            }
            0x98 => {
                self.a = self.y;
                self.update_zn(self.a);
                2
            }
            0x9A => {
                self.sp = self.x;
                2
            }
            0xA0 => {
                self.y = self.fetch_byte();
                self.update_zn(self.y);
                2
            }
            0xA4 => {
                let addr = self.addr_zp();
                self.y = self.cpu_read(addr);
                self.update_zn(self.y);
                3
            }
            0xA8 => {
                self.y = self.a;
                self.update_zn(self.y);
                2
            }
            0xAA => {
                self.x = self.a;
                self.update_zn(self.x);
                2
            }
            0xAC => {
                let addr = self.addr_abs();
                self.y = self.cpu_read(addr);
                self.update_zn(self.y);
                4
            }
            0xB0 => self.branch(self.get_flag(FLAG_CARRY)),
            0xB4 => {
                let addr = self.addr_zpx();
                self.y = self.cpu_read(addr);
                self.update_zn(self.y);
                4
            }
            0xB8 => {
                self.set_flag(FLAG_OVERFLOW, false);
                2
            }
            0xBA => {
                self.x = self.sp;
                self.update_zn(self.x);
                2
            }
            0xBC => {
                let (addr, page, base) = self.addr_absx_with_base();
                if page {
                    let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                    let _ = self.cpu_read(dummy_addr);
                }
                self.y = self.cpu_read(addr);
                self.update_zn(self.y);
                4 + page as u32
            }
            0xC0 => {
                let value = self.fetch_byte();
                self.compare(self.y, value);
                2
            }
            0xC4 => {
                let addr = self.addr_zp();
                let value = self.cpu_read(addr);
                self.compare(self.y, value);
                3
            }
            0xC8 => {
                self.y = self.y.wrapping_add(1);
                self.update_zn(self.y);
                2
            }
            0xCC => {
                let addr = self.addr_abs();
                let value = self.cpu_read(addr);
                self.compare(self.y, value);
                4
            }
            0xD0 => self.branch(!self.get_flag(FLAG_ZERO)),
            0xD8 => {
                self.set_flag(FLAG_DECIMAL, false);
                2
            }
            0xE0 => {
                let value = self.fetch_byte();
                self.compare(self.x, value);
                2
            }
            0xE4 => {
                let addr = self.addr_zp();
                let value = self.cpu_read(addr);
                self.compare(self.x, value);
                3
            }
            0xE8 => {
                self.x = self.x.wrapping_add(1);
                self.update_zn(self.x);
                2
            }
            0xEA => 2,
            0xEB => {
                let value = self.fetch_byte();
                self.sbc(value);
                2
            }
            0xEC => {
                let addr = self.addr_abs();
                let value = self.cpu_read(addr);
                self.compare(self.x, value);
                4
            }
            0xF0 => self.branch(self.get_flag(FLAG_ZERO)),
            0xF8 => {
                self.set_flag(FLAG_DECIMAL, true);
                2
            }

            0x04 | 0x44 | 0x64 => {
                let addr = self.addr_zp();
                let _ = self.cpu_read(addr);
                3
            }
            0x14 | 0x34 | 0x54 | 0x74 | 0xD4 | 0xF4 => {
                let addr = self.addr_zpx();
                let _ = self.cpu_read(addr);
                4
            }
            0x0C => {
                let addr = self.addr_abs();
                let _ = self.cpu_read(addr);
                4
            }
            0x1C | 0x3C | 0x5C | 0x7C | 0xDC | 0xFC => {
                let (addr, page, base) = self.addr_absx_with_base();
                if page {
                    let dummy_addr = (base & 0xFF00) | (addr & 0x00FF);
                    let _ = self.cpu_read(dummy_addr);
                }
                let _ = self.cpu_read(addr);
                4 + page as u32
            }
            0x1A | 0x3A | 0x5A | 0x7A | 0xDA | 0xFA => 2,

            0x02 | 0x12 | 0x22 | 0x32 | 0x42 | 0x52 | 0x62 | 0x72 | 0x92 | 0xB2 | 0xD2 | 0xF2 => {
                self.halted = true;
                2
            }

            _ => {
                self.note_unknown_opcode(opcode, opcode_pc);
                2
            }
        }
    }

    fn addr_zp(&mut self) -> u16 {
        self.fetch_byte() as u16
    }

    fn addr_zpx(&mut self) -> u16 {
        let base = self.fetch_byte();
        let _ = self.cpu_read(base as u16);
        base.wrapping_add(self.x) as u16
    }

    fn addr_zpy(&mut self) -> u16 {
        let base = self.fetch_byte();
        let _ = self.cpu_read(base as u16);
        base.wrapping_add(self.y) as u16
    }

    fn addr_abs(&mut self) -> u16 {
        self.fetch_word()
    }

    fn addr_absx_with_base(&mut self) -> (u16, bool, u16) {
        let base = self.fetch_word();
        let addr = base.wrapping_add(self.x as u16);
        (addr, (base & 0xFF00) != (addr & 0xFF00), base)
    }

    fn addr_absy_with_base(&mut self) -> (u16, bool, u16) {
        let base = self.fetch_word();
        let addr = base.wrapping_add(self.y as u16);
        (addr, (base & 0xFF00) != (addr & 0xFF00), base)
    }

    fn addr_indx(&mut self) -> u16 {
        let zp = self.fetch_byte();
        let _ = self.cpu_read(zp as u16);
        let base = zp.wrapping_add(self.x);
        self.read_zp_u16(base)
    }

    fn addr_indy_with_base(&mut self) -> (u16, bool, u16) {
        let base = self.fetch_byte();
        let ptr = self.read_zp_u16(base);
        let addr = ptr.wrapping_add(self.y as u16);
        (addr, (ptr & 0xFF00) != (addr & 0xFF00), ptr)
    }

    fn read_zp_u16(&mut self, addr: u8) -> u16 {
        let lo = self.cpu_read(addr as u16) as u16;
        let hi = self.cpu_read(addr.wrapping_add(1) as u16) as u16;
        (hi << 8) | lo
    }

    fn branch(&mut self, condition: bool) -> u32 {
        let offset = self.fetch_byte() as i8;
        if condition {
            let old_pc = self.pc;
            let _ = self.cpu_read(old_pc);
            let new_pc = self.pc.wrapping_add(offset as i16 as u16);
            if (old_pc & 0xFF00) != (new_pc & 0xFF00) {
                let dummy_addr = (old_pc & 0xFF00) | (new_pc & 0x00FF);
                let _ = self.cpu_read(dummy_addr);
                self.pc = new_pc;
                4
            } else {
                self.pc = new_pc;
                3
            }
        } else {
            2
        }
    }

    fn ora(&mut self, value: u8) {
        self.a |= value;
        self.update_zn(self.a);
    }

    fn and(&mut self, value: u8) {
        self.a &= value;
        self.update_zn(self.a);
    }

    fn eor(&mut self, value: u8) {
        self.a ^= value;
        self.update_zn(self.a);
    }

    fn bit(&mut self, value: u8) {
        self.set_flag(FLAG_ZERO, (self.a & value) == 0);
        self.set_flag(FLAG_NEGATIVE, (value & 0x80) != 0);
        self.set_flag(FLAG_OVERFLOW, (value & 0x40) != 0);
    }

    fn compare(&mut self, register: u8, value: u8) {
        let result = register.wrapping_sub(value);
        self.set_flag(FLAG_CARRY, register >= value);
        self.update_zn(result);
    }

    fn adc(&mut self, value: u8) {
        let carry_in = if self.get_flag(FLAG_CARRY) {
            1u16
        } else {
            0u16
        };
        let a = self.a as u16;
        let b = value as u16;
        let result = a + b + carry_in;
        let out = result as u8;

        self.set_flag(FLAG_CARRY, result > 0xFF);
        self.set_flag(FLAG_OVERFLOW, ((self.a ^ out) & (value ^ out) & 0x80) != 0);

        self.a = out;
        self.update_zn(self.a);
    }

    fn sbc(&mut self, value: u8) {
        self.adc(value ^ 0xFF);
    }

    fn asl(&mut self, value: u8) -> u8 {
        self.set_flag(FLAG_CARRY, (value & 0x80) != 0);
        let result = value << 1;
        self.update_zn(result);
        result
    }

    fn lsr(&mut self, value: u8) -> u8 {
        self.set_flag(FLAG_CARRY, (value & 0x01) != 0);
        let result = value >> 1;
        self.update_zn(result);
        result
    }

    fn rol(&mut self, value: u8) -> u8 {
        let carry_in = if self.get_flag(FLAG_CARRY) { 1 } else { 0 };
        self.set_flag(FLAG_CARRY, (value & 0x80) != 0);
        let result = (value << 1) | carry_in;
        self.update_zn(result);
        result
    }

    fn ror(&mut self, value: u8) -> u8 {
        let carry_in = if self.get_flag(FLAG_CARRY) { 0x80 } else { 0 };
        self.set_flag(FLAG_CARRY, (value & 0x01) != 0);
        let result = (value >> 1) | carry_in;
        self.update_zn(result);
        result
    }
}

#[derive(Clone, Copy)]
enum RmwOp {
    Asl,
    Rol,
    Lsr,
    Ror,
    Dec,
    Inc,
}

#[derive(Clone, Copy)]
enum UnofficialRmwOp {
    Slo,
    Rla,
    Sre,
    Rra,
    Dcp,
    Isc,
}
