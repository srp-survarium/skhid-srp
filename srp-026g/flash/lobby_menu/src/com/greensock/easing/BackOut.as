package com.greensock.easing
{
   public final class BackOut extends Ease
   {
      
      public static var ease:BackOut = new BackOut();
      
      public function BackOut(overshoot:Number = 1.70158)
      {
         super();
         _p1 = overshoot;
      }
      
      override public function getRatio(progress:Number) : Number
      {
         return (progress = progress - 1) * progress * ((_p1 + 1) * progress + _p1) + 1;
      }
      
      public function config(overshoot:Number = 1.70158) : BackOut
      {
         return new BackOut(overshoot);
      }
   }
}

