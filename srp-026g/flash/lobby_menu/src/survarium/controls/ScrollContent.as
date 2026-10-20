package survarium.controls
{
   import fl.transitions.easing.Strong;
   import flash.display.DisplayObject;
   import flash.display.Sprite;
   import flash.events.Event;
   import flash.events.FocusEvent;
   import flash.events.MouseEvent;
   import flash.utils.getDefinitionByName;
   import scaleform.clik.constants.InvalidationType;
   import scaleform.clik.controls.ScrollBar;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.motion.Tween;
   
   public class ScrollContent extends UIComponent
   {
      
      public var scrolChildrens:Sprite = new Sprite();
      
      public var fromBottom:Boolean = false;
      
      public var autoScroll:Boolean = false;
      
      protected var _mask:Sprite = new Sprite();
      
      protected var _back:Sprite = new Sprite();
      
      protected var _pos:int = 0;
      
      protected var _margin:int = 1;
      
      protected var _items:Array = new Array();
      
      protected var _tween:Tween;
      
      protected var _scrollTween:Boolean = false;
      
      protected var _toPos:Boolean = false;
      
      protected var _max:int;
      
      protected var _elemntShift:int = 0;
      
      protected var _scrlBar:ScrollBar;
      
      protected var _targets:Array = new Array();
      
      protected var _current_target:int = 0;
      
      public function ScrollContent()
      {
         super();
         this._back.graphics.clear();
         this._back.graphics.beginFill(16777215,0);
         this._back.graphics.drawRect(0,0,10,10);
         this._back.graphics.endFill();
         this._mask.graphics.clear();
         this._mask.graphics.beginFill(16711935,0.5);
         this._mask.graphics.drawRect(0,0,10,10);
         this._mask.graphics.endFill();
         addChild(this._back);
         addChild(this.scrolChildrens);
         addChild(this._mask);
         this._back.mouseEnabled = false;
         this._back.mouseChildren = false;
         this._mask.mouseEnabled = false;
         this._mask.mouseChildren = false;
         this.mask = this._mask;
         addEventListener(MouseEvent.MOUSE_WHEEL,this.handleMouseWheel);
      }
      
      public function set scrollV(param1:int) : void
      {
         var _loc2_:int = 0;
         if(this.fromBottom && param1 == 0)
         {
            if(this.scrolChildrens.height < this._mask.height)
            {
               this.scrolChildrens.y = this._mask.height - this.scrolChildrens.height;
               return;
            }
         }
         this.scrolChildrens.y = -param1;
      }
      
      public function set elemntShift(param1:int) : void
      {
         _elemntShift = param1;
      }
      
      protected function set scrollPosition(param1:int) : void
      {
         if(!this._tween)
         {
            _scrollTween = true;
            _tween = new Tween(500,this._scrlBar,{position:param1},{
               ease:Strong.easeOut,
               onComplete:this.endTween
            });
         }
         else
         {
            this._tween.reset();
            this._tween.paused = false;
            _scrollTween = true;
            this._tween.props = {position:param1};
         }
      }
      
      protected function get scrollPosition() : int
      {
         return this._scrlBar.position;
      }
      
      override public function set width(param1:Number) : void
      {
         super.width = param1;
         this.updateElements(param1,height);
      }
      
      override public function set height(param1:Number) : void
      {
         super.height = param1;
         this.updateElements(width,param1);
      }
      
      public function addContent(param1:DisplayObject) : void
      {
         param1.addEventListener(FocusEvent.FOCUS_IN,this.onFF);
         this.scrolChildrens.addChild(param1);
         invalidateData();
      }
      
      public function removeContent(param1:DisplayObject) : void
      {
         param1.removeEventListener(FocusEvent.FOCUS_IN,this.onFF);
         this.scrolChildrens.removeChild(param1);
         invalidateData();
      }
      
      protected function onFF(param1:FocusEvent) : void
      {
         if(param1.currentTarget is UIComponent)
         {
            if(param1.currentTarget.y > -this.scrolChildrens.y + this._mask.height)
            {
               if(this._scrlBar && this._scrlBar.enabled)
               {
                  scrollPosition = param1.currentTarget.y - this._mask.height;
               }
            }
            else if(param1.currentTarget.y < -this.scrolChildrens.y)
            {
               if(this._scrlBar && this._scrlBar.enabled)
               {
                  scrollPosition = param1.currentTarget.y;
               }
            }
         }
      }
      
      protected function onSBarDragStart(param1:Event = null) : void
      {
         if(this._tween)
         {
            this._tween.reset();
            this._tween.paused = true;
         }
         _scrollTween = false;
      }
      
      protected function onSBarDragEnd(param1:Event = null) : void
      {
         _current_target = this.seekTarget();
         scrollPosition = this._targets[this._current_target];
      }
      
      protected function onTR(param1:Event) : void
      {
         this.scrollToCurrent();
      }
      
      protected function endTween(param1:* = null) : void
      {
         _current_target = this.seekTarget();
         _scrollTween = false;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         var _loc1_:Class = getDefinitionByName("SrvScrollBarSkinned") as Class;
         _scrlBar = new _loc1_();
         this._scrlBar.x = width - this._scrlBar.width - 3;
         addChild(this._scrlBar);
         this._scrlBar.focusTarget = this;
         this._scrlBar.visible = false;
         this._scrlBar.tabEnabled = false;
         this._scrlBar.setScrollProperties(-1,-1,-1);
         this._scrlBar.setActualSize(this._scrlBar.width,height);
         this._scrlBar.scrollTarget = this;
         this._scrlBar.addEventListener(Event.COMPLETE,this.onSBarDragEnd);
         this._scrlBar.addEventListener(Event.SELECT,this.onSBarDragStart);
         if(this._scrlBar.track)
         {
            this._scrlBar.track.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress,false,0,true);
         }
         if(this._scrlBar.upArrow)
         {
            this._scrlBar.upArrow.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress,false,0,true);
         }
         if(this._scrlBar.downArrow)
         {
            this._scrlBar.downArrow.addEventListener(MouseEvent.MOUSE_DOWN,this.handleTrackPress,false,0,true);
         }
      }
      
      override protected function draw() : void
      {
         super.draw();
         if(isInvalid(InvalidationType.DATA))
         {
            this.updateSize();
            this.create_targets();
            if(this.autoScroll)
            {
               _current_target = this.getLastTarget();
               scrollPosition = this._targets[this._current_target];
            }
            else
            {
               this.scrollToCurrent();
            }
         }
      }
      
      protected function updateElements(param1:int, param2:int) : void
      {
         if(this._mask)
         {
            this._mask.width = param1;
            this._mask.height = param2;
            this._back.width = param1;
            this._back.height = param2;
         }
      }
      
      protected function onResized(param1:Event) : void
      {
         if(this.mask)
         {
            this._mask.height = height;
         }
         this._scrlBar.setActualSize(width,this._mask.height);
      }
      
      protected function getLastTarget() : int
      {
         return this._targets.length - 1;
      }
      
      protected function seekTarget() : int
      {
         var _loc1_:int = 0;
         var _loc2_:int = 0;
         while(_loc2_ < this._targets.length)
         {
            if(this._scrlBar.position == this._targets[_loc2_])
            {
               return _loc2_;
            }
            if(this._targets[_loc2_] > this._scrlBar.position)
            {
               if(Math.abs(this._targets[_loc2_] - this._scrlBar.position) > Math.abs(this._targets[_loc1_] - this._scrlBar.position))
               {
                  return _loc1_;
               }
               return _loc2_;
            }
            _loc1_ = _loc2_;
            _loc2_++;
         }
         return _loc2_;
      }
      
      protected function handleMouseWheel(param1:MouseEvent) : void
      {
         var _loc2_:int = this._current_target;
         if(param1.delta > 0)
         {
            _current_target -= 1;
         }
         else
         {
            _current_target += 1;
         }
         if(this._current_target < 0)
         {
            _current_target = 0;
         }
         if(this._current_target > this._targets.length - 1)
         {
            _current_target = this._targets.length - 1;
         }
         if(this._scrlBar && this._scrlBar.enabled)
         {
            scrollPosition = this._targets[this._current_target];
         }
         if(_loc2_ != this._current_target)
         {
            param1.preventDefault();
            param1.stopImmediatePropagation();
         }
      }
      
      protected function scrollToCurrent() : void
      {
         _current_target = this.seekTarget();
      }
      
      protected function handleTrackPress(param1:MouseEvent) : void
      {
         if(this._scrlBar.thumb.y > this._scrlBar.mouseY)
         {
            _current_target -= 1;
         }
         else
         {
            _current_target += 1;
         }
         if(this._current_target < 0)
         {
            _current_target = 0;
         }
         if(this._current_target > this._targets.length - 1)
         {
            _current_target = this._targets.length - 1;
         }
         if(this._scrlBar && this._scrlBar.enabled)
         {
            scrollPosition = this._targets[this._current_target];
         }
         param1.stopImmediatePropagation();
      }
      
      protected function create_targets() : void
      {
         this._targets.length = 0;
         var _loc1_:Array = new Array();
         var _loc2_:uint = (this.scrolChildrens.numChildren);
         var _loc3_:uint = 0;
         var _loc4_:DisplayObject;
         while(_loc3_ < _loc2_)
         {
            _loc4_ = this.scrolChildrens.getChildAt(_loc3_) as DisplayObject;
            if(_loc4_)
            {
               _loc1_.push(_loc4_);
            }
            _loc3_++;
         }
         _loc1_.sortOn("y",Array.NUMERIC);
         this._targets.push(0);
         _loc2_ = _loc1_.length;
         _loc3_ = 0;
         while(_loc3_ < _loc2_)
         {
            _loc4_ = _loc1_[_loc3_];
            if(_loc4_.y + _loc4_.height - this._elemntShift > this._mask.height)
            {
               this._targets.push(_loc1_[_loc3_].y + _loc4_.height - this._mask.height - this._elemntShift);
            }
            _loc3_++;
         }
         if(this._current_target > this._targets.length)
         {
            _current_target = this._targets.length;
         }
      }
      
      protected function updateSize() : void
      {
         var _loc1_:uint;
         var _loc2_:Sprite;
         _max = 0;
         if(this._scrlBar && numChildren)
         {
            _loc1_ = 0;
            while(_loc1_ < this.scrolChildrens.numChildren)
            {
               _loc2_ = this.scrolChildrens.getChildAt(_loc1_) as Sprite;
               if(_loc2_)
               {
                  _max = Math.max(_loc2_.height + _loc2_.y - this._elemntShift,this._max);
               }
               _loc1_++;
            }
            _max -= this._mask.height;
            this._scrlBar.setScrollProperties(this._mask.height,0,this._max,1);
            if(this._max <= 0)
            {
               this._scrlBar.visible = false;
            }
            else
            {
               this._scrlBar.visible = true;
            }
            if(this._scrlBar.position > this._max)
            {
               this._scrlBar.position = this._max;
            }
            else
            {
               scrollV = this._scrlBar.position;
            }
         }
      }
   }
}

