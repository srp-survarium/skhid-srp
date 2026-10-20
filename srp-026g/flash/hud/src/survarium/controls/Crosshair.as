package survarium.controls
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.utils.clearTimeout;
   import flash.utils.setTimeout;
   
   public class Crosshair extends MovieClip
   {
      
      protected var _hit:MovieClip = new hitMark();
      
      protected var _framesCount:uint;
      
      protected var _fov:uint = 65;
      
      protected var _hrez:uint = 1024;
      
      protected var _timeOut:int = -1;
      
      protected var _ignoreNewValues:Boolean = false;
      
      protected var _maxDisp:Number = 50;
      
      public var crosshair_mc:MovieClip;
      
      public var damage1:MovieClip;
      
      public var damage2:MovieClip;
      
      public var damage3:MovieClip;
      
      public var damage4:MovieClip;
      
      public var bars:MovieClip;
      
      public function Crosshair()
      {
         super();
         this._hit.gotoAndStop(0);
         _framesCount = this.crosshair_mc.totalFrames;
         this.bars.addEventListener(Event.COMPLETE,this.onBarsAnimationComplete);
         this.calculateMaxDisp();
      }
      
      public function calculateMaxDisp() : void
      {
         _maxDisp = this._fov * (400 / this._hrez);
      }
      
      public function resetVideoOptions(fov:uint, hrez:uint) : void
      {
         _fov = fov;
         _hrez = hrez;
         this.calculateMaxDisp();
      }
      
      public function setCrosshair(disp:Number) : void
      {
         var cdisp:Number = Math.min(disp,this._maxDisp);
         var frame:int = Math.ceil(this._framesCount * (cdisp / this._maxDisp));
         this.crosshair_mc.gotoAndStop(frame);
      }
      
      public function setDamage(value:Number, delay:uint, killed:Boolean, debug:Boolean) : void
      {

         if(this._ignoreNewValues)
         {
            if(killed)
            {
               _ignoreNewValues = false;
            }
            else
            {
               return;
            }
         }
         var container:MovieClip = this.bars.barsContainer;
         var frames:int = container.damage1.totalFrames - 1;
         this.bars.gotoAndStop(1);
         var frame:uint = Math.ceil(value * frames) + 1;
         function updateParts(value:MovieClip):void
         {
            if(value.currentFrame > 1)
            {
               value.part_1.gotoAndStop(1);
            }
            if(value.currentFrame > 2)
            {
               value.part_2.gotoAndStop(1);
            }
            if(value.currentFrame > 3)
            {
               value.part_3.gotoAndStop(1);
            }
         }
         var lastPart:String;
         var hitTex:MovieClip;
         var dir:Number;
         trace("++++++++++++setDamage::",frame);
         container.damage1.gotoAndStop(frame);
         container.damage2.gotoAndStop(frame);
         container.damage3.gotoAndStop(frame);
         container.damage4.gotoAndStop(frame);
         trace("++++++++++++null::");
         if(frame > 1)
         {
            updateParts(container.damage1);
            updateParts(container.damage2);
            updateParts(container.damage3);
            updateParts(container.damage4);
         }
         trace("++++++++++++killed::");
         if(killed && frame >= 2)
         {
            _ignoreNewValues = true;
            lastPart = "part_" + String(frame - 1);
            trace("lastPart::",lastPart);
            container.damage1[lastPart].gotoAndStop(2);
            container.damage2[lastPart].gotoAndStop(2);
            container.damage3[lastPart].gotoAndStop(2);
            container.damage4[lastPart].gotoAndStop(2);
         }
         if(debug)
         {
            hitTex = new HitText();
            addChild(hitTex);
            hitTex.x = -106;
            hitTex.y = -51;
            dir = 90 - (10 + Math.random() * 25) * (Math.random() > 0.5 ? -1 : 1);
            hitTex.startMove(dir,value * 100,killed);
         }
         if(this._timeOut != -1)
         {
            clearTimeout(this._timeOut);
            _timeOut = -1;
         }
         _timeOut = setTimeout(this.playBarsHide,delay);
      }
      
      private function playBarsHide() : void
      {
         this.bars.gotoAndPlay(1);
      }
      
      private function onBarsAnimationComplete(e:Event) : void
      {
         _ignoreNewValues = false;
      }
      
      public function hitEnemy() : void
      {
         if(!this._hit.parent)
         {
            this._hit.gotoAndPlay(2);
            addChild(this._hit);
         }
         else if(this._hit.currentFrame < 5)
         {
            this._hit.gotoAndPlay(1);
         }
         else
         {
            this._hit.gotoAndPlay(2);
         }
      }
   }
}

