package scaleform.clik.utils
{
   import flash.display.DisplayObject;
   import flash.display.DisplayObjectContainer;
   import flash.display.Sprite;
   import flash.display.Stage;
   import flash.events.Event;
   import flash.events.EventDispatcher;
   import flash.text.TextField;
   import scaleform.clik.constants.ConstrainMode;
   import scaleform.clik.core.UIComponent;
   import scaleform.clik.events.ResizeEvent;
   
   public class Constraints extends EventDispatcher
   {
      
      public static const LEFT:uint = 1;
      
      public static const RIGHT:uint = 2;
      
      public static const TOP:uint = 4;
      
      public static const BOTTOM:uint = 8;
      
      public static var ALL:uint = LEFT | RIGHT | TOP | BOTTOM;
      
      public static const CENTER_H:uint = 16;
      
      public static const CENTER_V:uint = 32;
      
      public var scope:DisplayObject;
      
      public var scaleMode:String = "counterScale";
      
      public var parentXAdjust:Number = 1;
      
      public var parentYAdjust:Number = 1;
      
      protected var elements:Object;
      
      protected var elementCount:int = 0;
      
      protected var parentConstraints:Constraints;
      
      public var lastWidth:Number = NaN;
      
      public var lastHeight:Number = NaN;
      
      public function Constraints(scopeValue:Sprite, scaleModeValue:String = "counterScale")
      {
         super();
         scope = scopeValue;
         scaleMode = scaleModeValue;
         elements = {};
         lastWidth = scopeValue.width;
         lastHeight = scopeValue.height;
         scopeValue.addEventListener(Event.ADDED_TO_STAGE,this.handleScopeAddedToStage,false,0,true);
         scopeValue.addEventListener(Event.REMOVED_FROM_STAGE,this.handleScopeAddedToStage,false,0,true);
      }
      
      public function addElement(name:String, clip:DisplayObject, edges:uint) : void
      {
         if(clip == null)
         {
            return;
         }
         var w:Number = this.scope.width;
         var h:Number = this.scope.height;
         if(this.scope.parent != null && this.scope.parent is Stage)
         {
            w = this.scope.stage.stageWidth;
            h = this.scope.stage.stageHeight;
         }
         var element:ConstrainedElement = new ConstrainedElement(clip,edges,clip.x,clip.y,w / this.scope.scaleX - (clip.x + clip.width),h / this.scope.scaleY - (clip.y + clip.height),clip.scaleX,clip.scaleY);
         if(this.elements[name] == null)
         {
            ++this.elementCount;
         }
         this.elements[name] = element;
      }
      
      public function removeElement(name:String) : void
      {
         if(this.elements[name] != null)
         {
            --this.elementCount;
         }
         delete this.elements[name];
      }
      
      public function removeAllElements() : void
      {
         var name:String;
         for(name in this.elements)
         {
            if(this.elements[name] is ConstrainedElement)
            {
               --this.elementCount;
               delete this.elements[name];
            }
         }
      }
      
      public function getElement(name:String) : ConstrainedElement
      {
         return this.elements[name] as ConstrainedElement;
      }
      
      public function updateElement(name:String, clip:DisplayObject) : void
      {
         if(clip == null)
         {
            return;
         }
         var element:ConstrainedElement = this.elements[name] as ConstrainedElement;
         if(element == null)
         {
            return;
         }
         element.clip = clip;
      }
      
      public function getXAdjust() : Number
      {
         if(this.scaleMode == ConstrainMode.REFLOW)
         {
            return this.parentXAdjust;
         }
         return this.parentXAdjust / this.scope.scaleX;
      }
      
      public function getYAdjust() : Number
      {
         if(this.scaleMode == ConstrainMode.REFLOW)
         {
            return this.parentYAdjust;
         }
         return this.parentYAdjust / this.scope.scaleY;
      }
      
      public function update(width:Number, height:Number) : void
      {
         lastWidth = width;
         lastHeight = height;
         if(this.elementCount == 0)
         {
            return;
         }
         var xAdjust:Number = this.getXAdjust();
         var yAdjust:Number = this.getYAdjust();
         var counterScale:Boolean = this.scaleMode == ConstrainMode.COUNTER_SCALE;
         var n:String;
         var element:ConstrainedElement;
         var edges:uint;
         var clip:DisplayObject;
         var nw:Number;
         var nh:Number;
         for(n in this.elements)
         {
            element = this.elements[n] as ConstrainedElement;
            edges = element.edges;
            clip = element.clip;
            if(counterScale)
            {
               clip.scaleX = element.scaleX * xAdjust;
               clip.scaleY = element.scaleY * yAdjust;
               if((edges & Constraints.CENTER_H) == 0)
               {
                  if((edges & Constraints.LEFT) > 0)
                  {
                     clip.x = element.left * xAdjust;
                     if((edges & Constraints.RIGHT) > 0)
                     {
                        nw = width - element.left - element.right;
                        if(!(clip is TextField))
                        {
                           nw *= xAdjust;
                        }
                        clip.width = nw;
                     }
                  }
                  else if((edges & Constraints.RIGHT) > 0)
                  {
                     clip.x = (width - element.right) * xAdjust - clip.width;
                  }
               }
               if((edges & Constraints.CENTER_V) == 0)
               {
                  if((edges & Constraints.TOP) > 0)
                  {
                     clip.y = element.top * yAdjust;
                     if((edges & Constraints.BOTTOM) > 0)
                     {
                        nh = height - element.top - element.bottom;
                        if(!(clip is TextField))
                        {
                           nh *= yAdjust;
                        }
                        clip.height = nh;
                     }
                  }
                  else if((edges & Constraints.BOTTOM) > 0)
                  {
                     clip.y = (height - element.bottom) * yAdjust - clip.height;
                  }
               }
            }
            else
            {
               if((edges & Constraints.CENTER_H) == 0 && (edges & Constraints.RIGHT) > 0)
               {
                  if((edges & Constraints.LEFT) > 0)
                  {
                     clip.width = width - element.left - element.right;
                  }
                  else
                  {
                     clip.x = width - clip.width - element.right;
                  }
               }
               if((edges & Constraints.CENTER_V) == 0 && (edges & Constraints.BOTTOM) > 0)
               {
                  if((edges & Constraints.TOP) > 0)
                  {
                     clip.height = height - element.top - element.bottom;
                  }
                  else
                  {
                     clip.y = height - clip.height - element.bottom;
                  }
               }
               if(clip is UIComponent)
               {
                  (clip as UIComponent).validateNow();
               }
            }
            if((edges & Constraints.CENTER_H) > 0)
            {
               clip.x = width * 0.5 * xAdjust - clip.width * 0.5;
            }
            if((edges & Constraints.CENTER_V) > 0)
            {
               clip.y = height * 0.5 * yAdjust - clip.height * 0.5;
            }
         }
         if(!counterScale)
         {
            this.scope.scaleX = this.parentXAdjust;
            this.scope.scaleY = this.parentYAdjust;
         }
         if(hasEventListener(ResizeEvent.RESIZE))
         {
            dispatchEvent(new ResizeEvent(ResizeEvent.RESIZE,xAdjust,yAdjust));
         }
      }
      
      override public function toString() : String
      {
         var l:uint = (this.elements.length);
         var str:String = "[CLIK Constraints (" + l + ")]";
         var n:String;
         for(n in this.elements)
         {
            str += "\n\t" + n + ": " + this.elements[n].toString();
         }
         return str;
      }
      
      protected function handleScopeAddedToStage(event:Event) : void
      {
         this.addToParentConstraints();
      }
      
      protected function addToParentConstraints() : void
      {
         if(this.parentConstraints != null)
         {
            this.parentConstraints.removeEventListener(ResizeEvent.RESIZE,this.handleParentConstraintsResize);
         }
         parentConstraints = null;
         var p:DisplayObjectContainer = this.scope.parent;
         if(p == null)
         {
            return;
         }
         while(p != null)
         {
            if(p.hasOwnProperty("constraints"))
            {
               parentConstraints = p["constraints"] as Constraints;
               if(this.parentConstraints != null && this.parentConstraints.scaleMode == ConstrainMode.REFLOW)
               {
                  return;
               }
               if(this.parentConstraints != null && this.scaleMode == ConstrainMode.REFLOW)
               {
                  return;
               }
               if(this.parentConstraints != null)
               {
                  this.parentConstraints.addEventListener(ResizeEvent.RESIZE,this.handleParentConstraintsResize,false,0,true);
                  break;
               }
            }
            p = p.parent;
         }
         if(this.parentConstraints != null)
         {
            parentXAdjust = this.parentConstraints.getXAdjust();
            parentYAdjust = this.parentConstraints.getYAdjust();
         }
      }
      
      protected function handleParentConstraintsResize(event:ResizeEvent) : void
      {
         parentXAdjust = event.scaleX;
         parentYAdjust = event.scaleY;
         this.update(this.lastWidth,this.lastHeight);
      }
   }
}

