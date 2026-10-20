package com.greensock.core
{
   public final class PropTween
   {
      
      public var t:Object;
      
      public var p:String;
      
      public var s:Number;
      
      public var c:Number;
      
      public var f:Boolean;
      
      public var pr:int;
      
      public var pg:Boolean;
      
      public var n:String;
      
      public var r:Boolean;
      
      public var _next:PropTween;
      
      public var _prev:PropTween;
      
      public function PropTween(target:Object, property:String, start:Number, change:Number, name:String, isPlugin:Boolean, next:PropTween = null, priority:int = 0)
      {
         super();
         t = target;
         p = property;
         s = start;
         c = change;
         n = name;
         f = target[property] is Function;
         pg = isPlugin;
         if(next)
         {
            next._prev = this;
            _next = next;
         }
         pr = priority;
      }
   }
}

