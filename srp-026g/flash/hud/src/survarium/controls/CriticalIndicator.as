package survarium.controls
{
   import flash.display.MovieClip;
   import flash.geom.ColorTransform;
   import flash.text.TextField;
   
   public class CriticalIndicator extends MovieClip
   {
      
      public var rHand:MovieClip;
      
      public var lHand:MovieClip;
      
      public var rLeg:MovieClip;
      
      public var lLeg:MovieClip;
      
      public var head:MovieClip;
      
      public var body:MovieClip;
      
      public var radInd:MovieClip;
      
      public var toxInd:MovieClip;
      
      public var bleedInd:MovieClip;
      
      public var tempTx:TextField;
      
      protected var _bodyParts:Array;
      
      protected var _bodyStats:Array;
      
      protected var _critCount:uint = 0;
      
      protected var _radiationFramesCount:uint;
      
      protected var _toxicFramesCount:uint;
      
      protected var _bleedFramesCount:uint;
      
      public function CriticalIndicator()
      {
         _bodyParts = ["","lHand","rHand","lLeg","rLeg","head","body"];
         _bodyStats = [1,1,1,1,1,1,1];
         super();
         stop();
         _radiationFramesCount = this.radInd.totalFrames;
         _toxicFramesCount = this.toxInd.totalFrames;
         _bleedFramesCount = this.bleedInd.totalFrames;
         this.head.gotoAndStop(1);
         this.body.gotoAndStop(1);
         this.rHand.gotoAndStop(1);
         this.lHand.gotoAndStop(1);
         this.rLeg.gotoAndStop(1);
         this.lLeg.gotoAndStop(1);
         visible = false;
         this.radInd.visible = this.toxInd.visible = this.bleedInd.visible = false;
         for(var i:uint = 1; i < this._bodyParts.length; i++)
         {
            var partRenderer:MovieClip;
            var redMC:MovieClip;
            var yellowMC:MovieClip;
            partRenderer = this[this._bodyParts[i]];
            redMC = partRenderer.mc_red;
            yellowMC = partRenderer.mc_yellow;
            trace("clean::",this[this._bodyParts[i]],this._bodyParts[i],redMC,yellowMC);
            if(redMC && yellowMC)
            {
               redMC.alpha = 0;
               yellowMC.alpha = 0;
            }
         }
      }
      
      public function setRadiation(vv:Number) : void
      {
         this.radInd.visible = vv != 0;
      }
      
      public function setToxic(vv:Number) : void
      {
         this.toxInd.visible = vv != 0;
      }
      
      public function setBleeding(vv:Number) : void
      {
         this.bleedInd.visible = vv != 0;
      }
      
      public function partHP(part_id:uint, value:Number, breakPoint:Number) : void
      {
         var partRenderer:MovieClip = this[this._bodyParts[part_id]];
         if(!partRenderer)
         {
            trace("WrongPartId",part_id);
            this.tempTx.text = "wrongPartId:: " + String(part_id);
            return;
         }
         this._bodyStats[part_id] = value;
         var GREEN_LIMIT:uint = 180;
         var green:Number = Math.min(value / breakPoint * GREEN_LIMIT,GREEN_LIMIT);
         var colorTransform:ColorTransform = partRenderer.transform.colorTransform;
         colorTransform.color = this.getColor(255,green,0);
         if(value < breakPoint)
         {
            colorTransform.alphaMultiplier = 0.5;
         }
         else
         {
            colorTransform.alphaMultiplier = 0.5 * ((1 - value) / (1 - breakPoint));
         }
         partRenderer.transform.colorTransform = colorTransform;
         for(var i:uint = 0; i < this._bodyStats.length; i++)
         {
            if(this._bodyStats[i] != 1)
            {
               if(this._critCount == 0)
               {
                  visible = true;
                  _critCount = 1;
                  trace("partHP::",visible);
               }
               return;
            }
         }
         if(this._critCount == 1)
         {
            visible = false;
            _critCount = 0;
            trace("partHP::",visible);
         }
      }
      
      protected function getColor(r:uint, g:uint, b:uint) : uint
      {
         var color:uint = r << 16 | g << 8 | b;
         return color;
      }
      
      public function crit(part_id:uint, value:uint) : void
      {
         var newValue:Number;
         if(value == 0)
         {
            newValue = 1;
         }
         else if(value == 1)
         {
            newValue = 0.4;
         }
         else if(value == 2)
         {
            newValue = 0;
         }
         this.partHP(part_id,newValue,0.5);
      }
      
      public function heal(part_id:uint) : void
      {
         if(this._bodyStats[part_id] == false)
         {
            return;
         }
         this._bodyStats[part_id] = false;
         this[this._bodyParts[part_id]].gotoAndPlay("off");
      }
      
      public function showCrit() : void
      {
         if(this._critCount == 0)
         {
            this.gotoAndPlay("on");
         }
         ++this._critCount;
      }
      
      public function hideCrit() : void
      {
         --this._critCount;
         if(this._critCount == 0)
         {
            this.gotoAndPlay("off");
         }
      }
   }
}

