package survarium.ammo
{
   import flash.display.MovieClip;
   import flash.events.Event;
   import flash.text.TextField;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.core.UIComponent;
   
   public class AmmoIndicator extends UIComponent
   {
      
      public var ammoAmmount:TextField;
      
      public var clipAmmount:TextField;
      
      public var clipAmmount2:TextField;
      
      public var primIndicator:MovieClip;
      
      public var setIndicator:MovieClip;
      
      public var secIndicator:MovieClip;
      
      public var cQueueSize:MovieClip;
      
      public var statminaProgress:MovieClip;
      
      protected const SELECTED_AMMO_FRAME:uint = 2;
      
      protected const DEFAULLT_AMMO_FRAME:uint = 1;
      
      protected const SELECTED_ALPHA:uint = 1;
      
      protected const DEFAULLT_ALPHA:Number = 0.7;
      
      protected const RENDERER_WIDTH:int = 22;
      
      protected var _ammoAmmount:TextField;
      
      protected var _currentAmmo:uint;
      
      protected var _activeAmmo:int = -1;
      
      protected var _totalList:Array;
      
      protected var _ammolist:Array = new Array();
      
      protected var _clip:uint = 0;
      
      protected var _selectedAmmo:int = -1;
      
      protected var renderers:Array = [];
      
      public function AmmoIndicator()
      {
         super();
         _ammoAmmount = this.ammoAmmount;
         this.clipAmmount2.alpha = 0.4;
         this.ammoAmmount.defaultTextFormat = this.ammoAmmount.getTextFormat();
         this.clipAmmount2.defaultTextFormat = this.clipAmmount2.getTextFormat();
         this.clipAmmount.defaultTextFormat = this.clipAmmount.getTextFormat();
      }
      
      public function setAmmoClip(amount:uint) : void
      {
         var count:int;
         if(!this._totalList)
         {
            return;
         }
         _clip = amount;
         var str:String = "";
         if(this._totalList[this._selectedAmmo])
         {
            this.ammoAmmount.text = amount.toString();
            count = (this._totalList[this._selectedAmmo].count);
         }
         else
         {
            this.ammoAmmount.text = "0";
            count = 0;
         }
         if(count == 0)
         {
            str = "000";
         }
         else if(count < 10)
         {
            str = "00";
         }
         else if(count > 9 && count < 100)
         {
            str = "0";
         }
         this.clipAmmount2.text = str;
         this.clipAmmount.text = count ? count.toString() : "";
         this.clipAmmount.width = this.clipAmmount.textWidth + 3;
         this.clipAmmount.x = this.clipAmmount2.x + this.clipAmmount2.textWidth;
      }
      
      public function setAmmo(value:Array) : void
      {
         _totalList = value;
         invalidateData();
      }
      
      public function setSelectedAmmo(value:uint) : void
      {
         _selectedAmmo = value;
         invalidateData();
      }
      
      public function setActiveAmmo(uid:uint) : void
      {
         _activeAmmo = uid;
         invalidateData();
      }
      
      public function setQueueSize(uid:uint) : void
      {
         if(!this._totalList)
         {
            return;
         }
         if(this.cQueueSize.currentFrame != uid)
         {
            this.cQueueSize.gotoAndStop(uid);
         }
      }
      
      protected function updateAmmoIndicator() : void
      {
         var resAmmo:AmmoRenderer;
         if(!this._totalList)
         {
            return;
         }
         for(var i:int = (this._totalList.length); i < this._ammolist.length; i++)
         {
            resAmmo = this._ammolist[i];
            if(resAmmo)
            {
               removeChild(resAmmo);
            }
            if(resAmmo)
            {
               resAmmo.removeEventListener(Event.RESIZE,this.updatePos);
            }
            this._ammolist.splice(i,1);
            i--;
         }
         renderers = [];
         var resX:uint = 0;
         var maxSize:int = 0;
         var label:String;
         var resFinalWidth:int;
         var isNew:Boolean;
         for(i = this._totalList.length - 1; i >= 0; i--)
         {
            if(!this._ammolist[i])
            {
               resAmmo = new ResAmmo();
               this._ammolist[i] = resAmmo;
               addChild(resAmmo);
               resAmmo.addEventListener(Event.RESIZE,this.updatePos);
            }
            else
            {
               resAmmo = this._ammolist[i];
            }
            label = this._totalList[i].name != undefined ? this._totalList[i].name : "null";
            if(this._selectedAmmo == i)
            {
               this.renderers.push(resAmmo);
               isNew = resAmmo.label != label;
               if(isNew)
               {
                  resAmmo.label = label;
               }
               trace("AI::selected::",resAmmo.currentFrame,resAmmo.label);
               if(resAmmo.state != AmmoRenderer.STATE_FULL || isNew)
               {
                  trace("playIn");
                  resAmmo.playIn();
               }
            }
            else
            {
               this.renderers.push(resAmmo);
               resAmmo.label = label;
               trace("AI::non selected::",resAmmo.currentFrame,resAmmo.label);
               if(resAmmo.state != AmmoRenderer.STATE_THIN)
               {
                  trace("playOut");
                  resAmmo.playOut();
               }
            }
            if(resAmmo.ammoTypeIcon.currentFrame != this._totalList[i].icon + 1)
            {
               resAmmo.ammoTypeIcon.gotoAndStop(this._totalList[i].icon + 1);
            }
            resAmmo.y = this.cQueueSize.y;
            resFinalWidth = (resAmmo.getFinalWidth());
            if(maxSize < resFinalWidth)
            {
               maxSize = resFinalWidth;
            }
            trace("resFinalWidth::",resFinalWidth,maxSize);
         }
         this.cQueueSize.x = Math.min(204 - (this.renderers.length * 15 + maxSize - 15),115);
         trace("cQueueSizePos::",this.cQueueSize.x);
         this.updatePos();
         this.setAmmoClip(this._clip);
      }
      
      protected function updatePos(e:Event = null) : void
      {
         var resX:uint = 0;
         for(var i:int = this.renderers.length - 1; i >= 0; i--)
         {
            var renderer:MovieClip;
            renderer = this.renderers[i];
            trace("renderer.width",i,renderer.width,resX);
            renderer.x = 234 - resX - renderer.width;
            resX += renderer.width + 2;
         }
      }
      
      override protected function draw() : void
      {
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateAmmoIndicator();
         }
      }
   }
}

