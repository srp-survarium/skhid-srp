package survarium.controls
{
   import flash.display.MovieClip;
   import flash.display.Sprite;
   import flash.text.TextField;
   import flash.utils.Timer;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.DragSlot;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.data.ListData;
   import scaleform.clik.interfaces.IListItemRenderer;
   import survarium.Util;
   import survarium.core.Icon;
   
   public class QuickItemRenderer extends DragSlot implements IListItemRenderer
   {
      
      protected var _index:uint = 0;
      
      protected var _icon:uint;
      
      protected var _timer:Timer = new Timer(1000);
      
      protected var _timer_text:Timer = new Timer(1000);
      
      protected var _owner:UIComponent;
      
      protected var _selectable:Boolean = true;
      
      protected var _selected:Boolean = false;
      
      protected var _highlighted:Boolean = false;
      
      protected var _run_st:int = 0;
      
      protected var _run_end:int = 0;
      
      public var keyBind:TextField;
      
      public var count:TextField;
      
      public var cooldDownTx:TextField;
      
      public var overCooldown:MovieClip;
      
      public var count_timer:TextField;
      
      public var artifact_background:Sprite;
      
      public var timerBack:MovieClip;
      
      public var background:MovieClip;
      
      public var glow:MovieClip;
      
      private var _activeRing:Sprite = new Sprite();

      protected var _lastCount:int = -1;
      
      protected var _lastTimer:int = -1;
      
      protected var _lastId:int = -1;
      
      protected var _bitmapColored:Boolean = true;
      
      public function QuickItemRenderer()
      {
         super();
         _stateMap = {
            def:["default"],
            disabled:["disabled"],
            cooldown:["cooldown"]
         };
         this._timer.repeatCount = 10;
         this._timer.start();
         this.artifact_background.visible = false;
         _activeRing.name = "consumableActivityRing";
         _activeRing.mouseEnabled = false;
         _activeRing.mouseChildren = false;
         addChild(_activeRing);
         this.glow.gotoAndStop(1);
         if(contentCanvas)
         {
            (contentCanvas as MovieClip).gotoAndStop(1);
         }
      }
      
      override public function set content(value:Sprite) : void
      {
         var container:MovieClip;
         trace("QIR::setContetn:",name,contentCanvas,value);
         if(value != _content)
         {
            container = (contentCanvas as MovieClip).container;
            if(_content)
            {
               if(container.contains(_content))
               {
                  container.removeChild(_content);
               }
            }
            _content = value;
            if(_content == null)
            {
               return;
            }
            if(_content != this)
            {
               _content.x = 0;
               _content.y = 0;
               container.addChild(_content);
               _content.mouseChildren = false;
            }
         }
      }
      
      public function get owner() : UIComponent
      {
         return this._owner;
      }
      
      public function set owner(value:UIComponent) : void
      {
         _owner = value;
      }
      
      public function get selectable() : Boolean
      {
         return this._selectable;
      }
      
      public function set selectable(value:Boolean) : void
      {
         _selectable = value;
      }
      
      public function get highlighted() : Boolean
      {
         return this._highlighted;
      }
      
      public function set highlighted(value:Boolean) : void
      {
         _highlighted = value;
         this.setState("out");
      }
      
      public function get selected() : Boolean
      {
         return this._selected;
      }
      
      public function set selected(value:Boolean) : void
      {
         _selected = value;
      }
      
      public function set disabled(value:Boolean) : void
      {
         enabled = !value;
         if(value == true)
         {
            this.setState("disabled");
         }
         else
         {
            this.setState("out");
         }
      }
      
      public function get disabled() : Boolean
      {
         return !enabled;
      }
      
      override public function set enabled(value:Boolean) : void
      {
         if(!value)
         {
            this.setState("disabled");
         }
         super.enabled = value;
      }
      
      override public function get data() : Object
      {
         return _data;
      }
      
      override public function set data(value:Object) : void
      {
         if(_data == value)
         {
            return;
         }
         _data = value;
         if(_data && _data.cooldown != undefined && _data.cooldown > 0)
         {
            this.setState("cooldown");
         }
         else
         {
            this.setState("def");
         }
         invalidateData();
      }
      
      public function get index() : uint
      {
         return this._index;
      }
      
      public function set index(value:uint) : void
      {
         _index = value;
         _descriptor = value;
      }
      
      public function setListData(listData:ListData) : void
      {
         index = listData.index;
         selected = listData.selected;
      }
      
      public function setData(data:Object) : void
      {
         this.data = data;
      }
      
      override public function toString() : String
      {
         return "[QuickItemRenderer " + name + "]";
      }
      
      protected function updateSlot() : void
      {
         _activeRing.graphics.clear();
         var _scale:Number;
         var sec:int;
         var contentContainer:MovieClip;
         var grayIcon:Icon;
         if(_data != null)
         {
            trace("QIR::",_data.is_artefact);
            enabled = _data.enabled;
            this.artifact_background.visible = _data.is_artefact == true;
            if(_data.enabled)
            {
               trace("sc_01");
               content = _data.icon;
               _scale = Math.min(110 / _content.width,110 / _content.height);
               contentCanvas.scaleX = _scale / scaleX;
               contentCanvas.scaleY = contentCanvas.scaleX;
               contentCanvas.y = 9;
               contentCanvas.x = -11;
            }
            else
            {
               content = null;
            }
            if(this.keyBind)
            {
               if(_data.enabled && _data.hotkey)
               {
                  this.keyBind.text = _data.hotkey;
               }
               else
               {
                  this.keyBind.text = "";
               }
            }
            if(this.count)
            {
               if(this._lastCount != _data.count)
               {
                  if(this._lastCount != -1 && this._lastCount != _data.count && this._lastId == _data.icon_id)
                  {
                     trace("contentContainer::playAsItemGlow");
                     if(contentCanvas)
                     {
                        (contentCanvas as MovieClip).gotoAndPlay("glow");
                     }
                  }
                  _lastCount = _data.count;
                  _lastId = _data.icon_id;
               }
               if(_data.count > 1 && _data.enabled)
               {
                  this.count.text = "x" + _data.count;
               }
               else
               {
                  this.count.text = "";
               }
            }
            if(_data.timer != undefined || _data.activitySeconds > 0)
            {
               sec = _data.activitySeconds > 0 ? int(_data.activitySeconds) : int(_data.timer);
               contentContainer = contentCanvas as MovieClip;
               trace("_lastTimer::",this._lastId,_data.icon_id,this._lastTimer,sec,"cooldown::",_data.cooldown);
               if(sec != 0)
               {
                  this.count_timer.text = Util.getZeroString(sec);
                  this.timerBack.visible = true;
                  trace("contentContainer::check_glow",contentContainer,contentContainer.currentFrame,this._lastTimer,sec,this._lastId,_data.icon_id);
                  if(_data.is_artefact && contentContainer && (contentContainer.currentFrame < 11 || contentContainer.currentFrame == 20))
                  {
                     if(this._lastTimer != sec)
                     {
                        if(this._lastTimer != -1 && this._lastTimer != sec && this._lastId == _data.icon_id)
                        {
                           trace("contentContainer::playAsArtifact");
                           if(contentContainer)
                           {
                              contentContainer.gotoAndPlay("glow_container");
                           }
                        }
                        _lastTimer = sec;
                        _lastId = _data.icon_id;
                     }
                  }
               }
               else
               {
                  trace("grayIcon::",_data.grayIcon,_data.is_artefact);
                  if(_data.grayIcon)
                  {
                     grayIcon = _data.grayIcon;
                     trace("sc_02");
                     content = grayIcon;
                  }
                  this.count_timer.text = "";
                  this.timerBack.visible = false;
                  if(contentContainer && contentContainer.currentFrame > 10)
                  {
                     contentContainer.gotoAndStop(1);
                  }
               }
            }
            else
            {
               contentContainer = contentCanvas as MovieClip;
               if(contentContainer && contentContainer.currentFrame > 10)
               {
                  contentContainer.gotoAndStop(1);
               }
               this.timerBack.visible = false;
            }
            if(_data.activitySeconds > 0)
            {
               drawActiveRing(Number(_data.activityFraction) / 100);
            }
            if(_data.cooldown != undefined && _data.cooldown > 0)
            {
               if(this.overCooldown)
               {
                  this.overCooldown.alpha = 0.3;
                  this.overCooldown.gotoAndStop(1);
                  this.overCooldown.rotation = -90;
                  this.drawSegment(this.overCooldown,6,-11,34,1,_data.cooldown / 100 * 360);
               }
               if(_data.grayIcon)
               {
                  grayIcon = _data.grayIcon;
                  trace("sc_04");
                  content = grayIcon;
               }
            }
            else
            {
               trace("!cooldnown::",_data.timer,_data.is_artefact,_data.icon_id,_data.grayIcon);
               if(_data.icon_id != 13 && _data.icon_id != 42 && _data.icon_id != 67 && _data.icon_id != 74 && _data.icon_id != 110 && _data.icon_id != 113)
               {
                  this.setState("def");
                  if(_data.grayIcon)
                  {
                     trace("sc_03");
                     content = _data.icon;
                  }
               }
            }
         }
         else
         {
            content = null;
            _lastCount = -1;
            _lastTimer = -1;
            _lastId = -1;
            if(this.keyBind)
            {
               this.keyBind.text = "";
            }
            if(this.count)
            {
               this.count.text = "";
            }
            if(this.count_timer)
            {
               this.count_timer.text = "";
            }
            if(this.timerBack)
            {
               this.timerBack.visible = false;
            }
         }
      }
      
      private function drawActiveRing(fraction:Number) : void
      {
         var cx:Number = -11;
         var cy:Number = -6;
         var radius:Number = 34;
         var end:Number = -Math.PI / 2 + Math.PI * 2 * Math.min(1,Math.max(0,fraction));
         _activeRing.graphics.lineStyle(2,0xFFFFFF,0.22);
         _activeRing.graphics.drawCircle(cx,cy,radius);
         _activeRing.graphics.lineStyle(3,0xFFFFFF,0.95);
         _activeRing.graphics.moveTo(cx,cy - radius);
         for(var angle:Number = -Math.PI / 2; angle < end; angle += 0.08)
            _activeRing.graphics.lineTo(cx + radius * Math.cos(angle),cy + radius * Math.sin(angle));
         _activeRing.graphics.lineTo(cx + radius * Math.cos(end),cy + radius * Math.sin(end));
      }

      private function drawSegment(target:Sprite, x:Number, y:Number, r:Number, aStart:Number, aEnd:Number, step:Number = 10) : void
      {
         var degreesPerRadian:Number = Math.PI / 180;
         aStart *= degreesPerRadian;
         aEnd *= degreesPerRadian;
         step *= degreesPerRadian;
         target.graphics.clear();
         target.graphics.moveTo(x,y);
         target.graphics.beginFill(16777215,1);
         for(var theta:Number = aStart; theta < aEnd; theta += Math.min(step,aEnd - theta))
         {
            target.graphics.lineTo(x + r * Math.cos(theta),y + r * Math.sin(theta));
         }
         target.graphics.lineTo(x + r * Math.cos(aEnd),y + r * Math.sin(aEnd));
         target.graphics.lineTo(x,y);
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         this.count.defaultTextFormat = this.count.getTextFormat();
         this.keyBind.defaultTextFormat = this.keyBind.getTextFormat();
         this.count_timer.defaultTextFormat = this.count_timer.getTextFormat();
      }
      
      override protected function setState(state:String) : void
      {
         if(!enabled)
         {
            return;
         }
         super.setState(state);
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateSlot();
         }
      }
   }
}

