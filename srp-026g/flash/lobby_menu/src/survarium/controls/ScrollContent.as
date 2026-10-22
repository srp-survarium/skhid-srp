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
      
      public function set scrollV(value:int) : void
      {
         var unused:int = 0;
         if(this.fromBottom && value == 0)
         {
            if(this.scrolChildrens.height < this._mask.height)
            {
               this.scrolChildrens.y = this._mask.height - this.scrolChildrens.height;
               return;
            }
         }
         this.scrolChildrens.y = -value;
      }
      
      public function set elemntShift(value:int) : void
      {
         _elemntShift = value;
      }
      
      protected function set scrollPosition(value:int) : void
      {
         if(!this._tween)
         {
            _scrollTween = true;
            _tween = new Tween(500,this._scrlBar,{position:value},{
               ease:Strong.easeOut,
               onComplete:this.endTween
            });
         }
         else
         {
            this._tween.reset();
            this._tween.paused = false;
            _scrollTween = true;
            this._tween.props = {position:value};
         }
      }
      
      protected function get scrollPosition() : int
      {
         return this._scrlBar.position;
      }
      
      override public function set width(value:Number) : void
      {
         super.width = value;
         this.updateElements(value,height);
      }
      
      override public function set height(value:Number) : void
      {
         super.height = value;
         this.updateElements(width,value);
      }
      
      public function addContent(child:DisplayObject) : void
      {
         child.addEventListener(FocusEvent.FOCUS_IN,this.onFF);
         this.scrolChildrens.addChild(child);
         invalidateData();
      }
      
      public function removeContent(child:DisplayObject) : void
      {
         child.removeEventListener(FocusEvent.FOCUS_IN,this.onFF);
         this.scrolChildrens.removeChild(child);
         invalidateData();
      }
      
      protected function onFF(event:FocusEvent) : void
      {
         if(event.currentTarget is UIComponent)
         {
            if(event.currentTarget.y > -this.scrolChildrens.y + this._mask.height)
            {
               if(this._scrlBar && this._scrlBar.enabled)
               {
                  scrollPosition = event.currentTarget.y - this._mask.height;
               }
            }
            else if(event.currentTarget.y < -this.scrolChildrens.y)
            {
               if(this._scrlBar && this._scrlBar.enabled)
               {
                  scrollPosition = event.currentTarget.y;
               }
            }
         }
      }
      
      protected function onSBarDragStart(event:Event = null) : void
      {
         if(this._tween)
         {
            this._tween.reset();
            this._tween.paused = true;
         }
         _scrollTween = false;
      }
      
      protected function onSBarDragEnd(event:Event = null) : void
      {
         _current_target = this.seekTarget();
         scrollPosition = this._targets[this._current_target];
      }
      
      protected function onTR(event:Event) : void
      {
         this.scrollToCurrent();
      }
      
      protected function endTween(tween:* = null) : void
      {
         _current_target = this.seekTarget();
         _scrollTween = false;
      }
      
      override protected function configUI() : void
      {
         super.configUI();
         var scrollBarClass:Class = getDefinitionByName("SrvScrollBarSkinned") as Class;
         _scrlBar = new scrollBarClass();
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
      
      protected function updateElements(newWidth:int, newHeight:int) : void
      {
         if(this._mask)
         {
            this._mask.width = newWidth;
            this._mask.height = newHeight;
            this._back.width = newWidth;
            this._back.height = newHeight;
         }
      }
      
      protected function onResized(event:Event) : void
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
         var prevIndex:int = 0;
         var i:int = 0;
         while(i < this._targets.length)
         {
            if(this._scrlBar.position == this._targets[i])
            {
               return i;
            }
            if(this._targets[i] > this._scrlBar.position)
            {
               if(Math.abs(this._targets[i] - this._scrlBar.position) > Math.abs(this._targets[prevIndex] - this._scrlBar.position))
               {
                  return prevIndex;
               }
               return i;
            }
            prevIndex = i;
            i++;
         }
         return i;
      }
      
      protected function handleMouseWheel(event:MouseEvent) : void
      {
         var prevTarget:int = this._current_target;
         if(event.delta > 0)
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
         if(prevTarget != this._current_target)
         {
            event.preventDefault();
            event.stopImmediatePropagation();
         }
      }
      
      protected function scrollToCurrent() : void
      {
         _current_target = this.seekTarget();
      }
      
      protected function handleTrackPress(event:MouseEvent) : void
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
         event.stopImmediatePropagation();
      }
      
      protected function create_targets() : void
      {
         this._targets.length = 0;
         var children:Array = new Array();
         var count:uint = (this.scrolChildrens.numChildren);
         var i:uint = 0;
         var child:DisplayObject;
         while(i < count)
         {
            child = this.scrolChildrens.getChildAt(i) as DisplayObject;
            if(child)
            {
               children.push(child);
            }
            i++;
         }
         children.sortOn("y",Array.NUMERIC);
         this._targets.push(0);
         count = children.length;
         i = 0;
         while(i < count)
         {
            child = children[i];
            if(child.y + child.height - this._elemntShift > this._mask.height)
            {
               this._targets.push(children[i].y + child.height - this._mask.height - this._elemntShift);
            }
            i++;
         }
         if(this._current_target > this._targets.length)
         {
            _current_target = this._targets.length;
         }
      }
      
      protected function updateSize() : void
      {
         var i:uint;
         var child:Sprite;
         _max = 0;
         if(this._scrlBar && numChildren)
         {
            i = 0;
            while(i < this.scrolChildrens.numChildren)
            {
               child = this.scrolChildrens.getChildAt(i) as Sprite;
               if(child)
               {
                  _max = Math.max(child.height + child.y - this._elemntShift,this._max);
               }
               i++;
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

