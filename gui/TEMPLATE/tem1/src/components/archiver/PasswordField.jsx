import React, { useState } from 'react';
import { Card, CardContent, CardHeader, CardTitle } from '@/components/ui/card';
import { Input } from '@/components/ui/input';
import { Progress } from '@/components/ui/progress';
import { Badge } from '@/components/ui/badge';
import { Eye, EyeOff, Lock, Shield } from 'lucide-react';

export default function PasswordField({ password, onPasswordChange, placeholder = "Пароль для защиты" }) {
  const [showPassword, setShowPassword] = useState(false);

  const getPasswordStrength = (pass) => {
    if (!pass) return { score: 0, label: 'Нет', color: 'bg-gray-500' };
    
    let score = 0;
    let checks = {
      length: pass.length >= 8,
      uppercase: /[A-Z]/.test(pass),
      lowercase: /[a-z]/.test(pass),
      numbers: /\d/.test(pass),
      symbols: /[!@#$%^&*(),.?":{}|<>]/.test(pass)
    };
    
    score = Object.values(checks).filter(Boolean).length;
    
    if (score <= 2) return { score, label: 'Слабый', color: 'bg-red-500' };
    if (score <= 3) return { score, label: 'Средний', color: 'bg-yellow-500' };
    if (score <= 4) return { score, label: 'Хороший', color: 'bg-blue-500' };
    return { score, label: 'Отличный', color: 'bg-green-500' };
  };

  const strength = getPasswordStrength(password);

  return (
    <Card className="bg-slate-800/30 border-slate-600 backdrop-blur-sm">
      <CardHeader className="pb-4">
        <CardTitle className="text-cyan-300 flex items-center gap-2">
          <Shield className="w-5 h-5" />
          Защита паролем
        </CardTitle>
      </CardHeader>
      
      <CardContent className="space-y-4">
        <div className="relative">
          <Lock className="absolute left-3 top-1/2 transform -translate-y-1/2 text-slate-400 w-4 h-4" />
          <Input
            type={showPassword ? "text" : "password"}
            value={password}
            onChange={(e) => onPasswordChange(e.target.value)}
            placeholder={placeholder}
            className="pl-10 pr-10 bg-slate-700/50 border-slate-600 text-slate-200"
          />
          <button
            type="button"
            onClick={() => setShowPassword(!showPassword)}
            className="absolute right-3 top-1/2 transform -translate-y-1/2 text-slate-400 hover:text-slate-300"
          >
            {showPassword ? <EyeOff className="w-4 h-4" /> : <Eye className="w-4 h-4" />}
          </button>
        </div>

        {password && (
          <div className="space-y-2">
            <div className="flex items-center justify-between">
              <span className="text-sm text-slate-300">Сила пароля</span>
              <Badge 
                variant="outline" 
                className={`${strength.color} text-white border-0`}
              >
                {strength.label}
              </Badge>
            </div>
            
            <Progress 
              value={(strength.score / 5) * 100} 
              className="h-2"
            />
            
            <div className="text-xs text-slate-400 space-y-1">
              <div className="flex items-center gap-2">
                <span className={password.length >= 8 ? 'text-green-400' : 'text-red-400'}>
                  {password.length >= 8 ? '✓' : '✗'}
                </span>
                Минимум 8 символов
              </div>
              <div className="flex items-center gap-2">
                <span className={/[A-Z]/.test(password) ? 'text-green-400' : 'text-red-400'}>
                  {/[A-Z]/.test(password) ? '✓' : '✗'}
                </span>
                Заглавные буквы
              </div>
              <div className="flex items-center gap-2">
                <span className={/\d/.test(password) ? 'text-green-400' : 'text-red-400'}>
                  {/\d/.test(password) ? '✓' : '✗'}
                </span>
                Цифры
              </div>
              <div className="flex items-center gap-2">
                <span className={/[!@#$%^&*(),.?":{}|<>]/.test(password) ? 'text-green-400' : 'text-red-400'}>
                  {/[!@#$%^&*(),.?":{}|<>]/.test(password) ? '✓' : '✗'}
                </span>
                Спецсимволы
              </div>
            </div>
          </div>
        )}
      </CardContent>
    </Card>
  );
}